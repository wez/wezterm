//! A fairly simple executor for futures that run on the GUI thread.
//! Ideally we'd use something like a tokio core for this, but that
//! would only work on X11 systems and we'd need to subvert more
//! of the winit event loop driving.
//! Instead we use winit Awakened events to fall out of the blocking
//! gui loop and tend to the queued futures.
//! The core dispatching portion of this code is derived from one
//! of the test cases in the futures-rs library which is licensed
//! under the MIT license and has this copyright:
//! Copyright (c) 2016 Alex Crichton
//! Copyright (c) 2017 The Tokio Authors

use failure::Error;
use futures::executor::{self, Notify, Spawn};
use futures::future::{ExecuteError, Executor};
use futures::{Async, Future};
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

const EXTERNAL_SPAWN: usize = !0usize;

pub trait CoreSender: Send {
    fn send(&self, idx: usize) -> Result<(), Error>;
    fn clone_sender(&self) -> Box<CoreSender>;
}

pub trait CoreReceiver {
    fn try_recv(&self) -> Result<usize, mpsc::TryRecvError>;
}

pub struct Core {
    tx: Box<CoreSender>,
    rx: Box<CoreReceiver>,
    notify: Arc<Notifier>,
    // Slab of running futures used to track what's running and what slots are
    // empty. Slot indexes are then sent along tx/rx above to indicate which
    // future is ready to get polled.
    tasks: RefCell<Vec<Slot>>,
    next_vacant: Cell<usize>,
    external_spawn: Arc<SpawningFromAnotherThread>,
}

#[derive(Default)]
struct SpawningFromAnotherThread {
    futures: Mutex<VecDeque<Box<Future<Item = (), Error = ()> + Send>>>,
}

pub struct Spawner {
    tx: Box<CoreSender>,
    external_spawn: Arc<SpawningFromAnotherThread>,
}

impl Spawner {
    pub fn spawn(&self, future: Box<dyn Future<Item = (), Error = ()> + Send>) {
        let mut futures = self.external_spawn.futures.lock().unwrap();
        futures.push_back(future);
        self.tx.send(EXTERNAL_SPAWN).unwrap();
    }
}

impl Clone for Spawner {
    fn clone(&self) -> Spawner {
        Spawner {
            tx: self.tx.clone_sender(),
            external_spawn: Arc::clone(&self.external_spawn),
        }
    }
}

enum Slot {
    Vacant { next_vacant: usize },
    RunningFuture(Option<Spawn<Box<Future<Item = (), Error = ()>>>>),
}

impl Core {
    pub fn new(tx: Box<CoreSender>, rx: Box<CoreReceiver>) -> Self {
        let tx2 = tx.clone_sender();
        Self {
            notify: Arc::new(Notifier {
                tx: Mutex::new(tx2),
            }),
            tx,
            rx,
            next_vacant: Cell::new(0),
            tasks: RefCell::new(Vec::new()),
            external_spawn: Arc::new(SpawningFromAnotherThread::default()),
        }
    }

    pub fn get_spawner(&self) -> Spawner {
        Spawner {
            tx: self.tx.clone_sender(),
            external_spawn: Arc::clone(&self.external_spawn),
        }
    }

    /// Spawn a future to be executed by a future call to `turn`.
    /// The future `f` provided will not be executed until the
    /// `turn` method is called.
    pub fn spawn<F>(&self, f: F)
    where
        F: Future<Item = (), Error = ()> + 'static,
    {
        self.spawn_impl(Box::new(f), true);
    }

    fn spawn_impl(&self, f: Box<Future<Item = (), Error = ()> + 'static>, do_tx: bool) -> usize {
        let idx = self.next_vacant.get();
        let mut tasks = self.tasks.borrow_mut();
        match tasks.get_mut(idx) {
            Some(&mut Slot::Vacant { next_vacant }) => {
                self.next_vacant.set(next_vacant);
            }
            Some(&mut Slot::RunningFuture(_)) => panic!("vacant points to running future"),
            None => {
                assert_eq!(idx, tasks.len());
                tasks.push(Slot::Vacant { next_vacant: 0 });
                self.next_vacant.set(idx + 1);
            }
        }
        tasks[idx] = Slot::RunningFuture(Some(executor::spawn(Box::new(f))));
        if do_tx {
            self.tx.send(idx).unwrap();
        }
        idx
    }

    fn spawn_external(&self) -> Option<usize> {
        let mut futures = self.external_spawn.futures.lock().unwrap();
        if let Some(future) = futures.pop_front() {
            Some(self.spawn_impl(future, false))
        } else {
            None
        }
    }

    /// "Turns" this event loop one tick.
    /// Does not block.
    /// Returns `false` if there were no futures in a known-ready state.
    pub fn turn(&self) -> bool {
        let task_id = match self.rx.try_recv() {
            Ok(task_id) if task_id == EXTERNAL_SPAWN => match self.spawn_external() {
                Some(task_id) => task_id,
                _ => return false,
            },
            Ok(task_id) => task_id,
            Err(mpsc::TryRecvError::Empty) => return false,
            Err(mpsc::TryRecvError::Disconnected) => panic!("futurecore rx Disconnected"),
        };

        // This may be a spurious wakeup so we're not guaranteed to have a
        // future associated with `task_id`, so do a fallible lookup.
        //
        // Note that we don't want to borrow `self.tasks` for too long so we
        // try to extract the future here and leave behind a tombstone future
        // which'll get replaced or removed later. This is how we support
        // spawn-in-run.
        let mut future = match self.tasks.borrow_mut().get_mut(task_id) {
            Some(&mut Slot::RunningFuture(ref mut future)) => future.take().unwrap(),
            Some(&mut Slot::Vacant { .. }) | None => return false,
        };

        // Drive this future forward. If it's done we remove it and if it's not
        // done then we put it back in the tasks array.
        let done = match future.poll_future_notify(&self.notify, task_id) {
            Ok(Async::Ready(())) | Err(()) => true,
            Ok(Async::NotReady) => false,
        };
        let mut tasks = self.tasks.borrow_mut();
        if done {
            tasks[task_id] = Slot::Vacant {
                next_vacant: self.next_vacant.get(),
            };
            self.next_vacant.set(task_id);
        } else {
            tasks[task_id] = Slot::RunningFuture(Some(future));
        }

        true
    }
}

impl<F> Executor<F> for Core
where
    F: Future<Item = (), Error = ()> + 'static,
{
    fn execute(&self, future: F) -> Result<(), ExecuteError<F>> {
        self.spawn(future);
        Ok(())
    }
}

struct Notifier {
    tx: Mutex<Box<CoreSender>>,
}

impl Notify for Notifier {
    fn notify(&self, id: usize) {
        self.tx.lock().unwrap().send(id).ok();
    }
}
