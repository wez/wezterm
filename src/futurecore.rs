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

use futures::{Async, Future};
use futures::executor::{self, Notify, Spawn};
use futures::future::{ExecuteError, Executor};
use std::cell::{Cell, RefCell};
use std::sync::{Arc, Mutex};
use std::sync::mpsc;

struct Core {
    tx: mpsc::Sender<usize>,
    rx: mpsc::Receiver<usize>,
    notify: Arc<Notifier>,
    // Slab of running futures used to track what's running and what slots are
    // empty. Slot indexes are then sent along tx/rx above to indicate which
    // future is ready to get polled.
    tasks: RefCell<Vec<Slot>>,
    next_vacant: Cell<usize>,
}

enum Slot {
    Vacant { next_vacant: usize },
    RunningFuture(Option<Spawn<Box<Future<Item = (), Error = ()>>>>),
}

impl Core {
    fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            notify: Arc::new(Notifier {
                tx: Mutex::new(tx.clone()),
            }),
            tx,
            rx,
            next_vacant: Cell::new(0),
            tasks: RefCell::new(Vec::new()),
        }
    }

    /// Spawn a future to be executed by a future call to `turn`.
    /// The future `f` provided will not be executed until the
    /// `turn` method is called.
    pub fn spawn<F>(&self, f: F)
    where
        F: Future<Item = (), Error = ()> + 'static,
    {
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
        self.tx.send(idx).unwrap();
    }

    /// "Turns" this event loop one tick.
    /// Does not block.
    /// Returns `false` if there were no futures in a known-ready state.
    fn turn(&self) -> bool {
        let task_id = match self.rx.try_recv() {
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
            Some(&mut Slot::Vacant { .. }) => return false,
            None => return false,
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

        return true;
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
    // TODO: it's pretty unfortunate to use a `Mutex` here where the `Sender`
    //       itself is basically `Sync` as-is. Ideally this'd use something like
    //       an off-the-shelf mpsc queue as well as `thread::park` and
    //       `Thread::unpark`.
    tx: Mutex<mpsc::Sender<usize>>,
}

impl Notify for Notifier {
    fn notify(&self, id: usize) {
        drop(self.tx.lock().unwrap().send(id));
    }
}
