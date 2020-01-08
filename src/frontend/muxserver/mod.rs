//! Implements the multiplexer server frontend
use crate::font::FontConfiguration;
use crate::frontend::{executor, front_end, FrontEnd};
use crate::mux::tab::Tab;
use crate::mux::window::WindowId;
use crate::mux::Mux;
use crate::server::listener::spawn_listener;
use anyhow::{bail, Error};
use log::info;
use promise::*;
use std::cell::{Cell, RefCell};
use std::future::Future;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::task::{Context, RawWaker, RawWakerVTable, Waker};

struct Task(pub std::pin::Pin<Box<dyn Future<Output = ()>>>);

enum Slot {
    Vacant { next_vacant: usize },
    Running(Option<Task>),
}

#[derive(Default)]
struct Tasks {
    tasks: RefCell<Vec<Slot>>,
    next_vacant: Cell<usize>,
}

impl Tasks {
    pub fn add_task(&self, task: Task) -> usize {
        let idx = self.next_vacant.get();
        let mut tasks = self.tasks.borrow_mut();
        match tasks.get_mut(idx) {
            Some(&mut Slot::Vacant { next_vacant }) => {
                self.next_vacant.set(next_vacant);
                tasks[idx] = Slot::Running(Some(task));
            }
            Some(&mut Slot::Running(_)) => panic!("vacant points to running task"),
            None => {
                assert_eq!(idx, tasks.len());
                tasks.push(Slot::Running(Some(task)));
                self.next_vacant.set(idx + 1);
            }
        }
        idx
    }

    pub fn poll_by_slot(&self, slot: usize) -> bool {
        let mut task = match self.tasks.borrow_mut().get_mut(slot) {
            Some(&mut Slot::Running(ref mut task)) => task.take().unwrap(),
            Some(&mut Slot::Vacant { .. }) | None => return false,
        };

        let waker = TaskWaker::new_waker(slot);
        let mut context = Context::from_waker(&waker);

        let done = task.0.as_mut().poll(&mut context).is_ready();

        let mut tasks = self.tasks.borrow_mut();
        if done {
            tasks[slot] = Slot::Vacant {
                next_vacant: self.next_vacant.get(),
            };
            self.next_vacant.set(slot);
        } else {
            tasks[slot] = Slot::Running(Some(task));
        }

        true
    }
}

struct TaskWaker(usize);

static VTBL: RawWakerVTable = RawWakerVTable::new(
    TaskWaker::waker_clone,
    TaskWaker::waker_wake,
    TaskWaker::waker_wake_by_ref,
    TaskWaker::waker_drop,
);

impl TaskWaker {
    fn new_waker(slot: usize) -> Waker {
        let raw = RawWaker::new(slot as *const (), &VTBL);
        unsafe { Waker::from_raw(raw) }
    }

    unsafe fn waker_clone(p: *const ()) -> RawWaker {
        RawWaker::new(p, &VTBL)
    }

    unsafe fn waker_wake(p: *const ()) {
        let id: usize = std::mem::transmute(p);
        wake_task_by_id(id);
    }

    unsafe fn waker_wake_by_ref(p: *const ()) {
        let id: usize = std::mem::transmute(p);
        wake_task_by_id(id);
    }

    unsafe fn waker_drop(_p: *const ()) {
        /* no action required */
    }
}

#[derive(Clone)]
struct MuxExecutor {
    tx: Sender<SpawnFunc>,
}

impl BasicExecutor for MuxExecutor {
    fn execute(&self, f: SpawnFunc) {
        self.tx.send(f).expect("MuxExecutor execute failed");
    }
}

impl Executor for MuxExecutor {
    fn clone_executor(&self) -> Box<dyn Executor> {
        Box::new(MuxExecutor {
            tx: self.tx.clone(),
        })
    }
}

pub struct MuxServerFrontEnd {
    tx: Sender<SpawnFunc>,
    rx: Receiver<SpawnFunc>,
    tasks: Tasks,
}

fn wake_task_by_id(id: usize) {
    executor().execute(Box::new(move || {
        let frontend = front_end().unwrap();
        let frontend = frontend
            .downcast_ref::<MuxServerFrontEnd>()
            .expect("mux server");
        frontend.tasks.poll_by_slot(id);
    }));
}

impl MuxServerFrontEnd {
    #[cfg_attr(feature = "cargo-clippy", allow(clippy::new_ret_no_self))]
    fn new(start_listener: bool) -> Result<Rc<dyn FrontEnd>, Error> {
        let (tx, rx) = mpsc::channel();

        if start_listener {
            spawn_listener()?;
        }
        Ok(Rc::new(Self {
            tx,
            rx,
            tasks: Tasks::default(),
        }))
    }

    pub fn try_new() -> Result<Rc<dyn FrontEnd>, Error> {
        Self::new(true)
    }

    pub fn new_null() -> Result<Rc<dyn FrontEnd>, Error> {
        Self::new(false)
    }

    pub fn spawn_task<F: std::future::Future<Output = ()> + 'static>(&self, future: F) {
        let id = self.tasks.add_task(Task(Box::pin(future)));
        wake_task_by_id(id);
    }
}

impl FrontEnd for MuxServerFrontEnd {
    fn executor(&self) -> Box<dyn Executor> {
        Box::new(MuxExecutor {
            tx: self.tx.clone(),
        })
    }

    fn low_pri_executor(&self) -> Box<dyn Executor> {
        self.executor()
    }

    fn run_forever(&self) -> Result<(), Error> {
        loop {
            match self.rx.recv() {
                Ok(func) => func(),
                Err(err) => bail!("while waiting for events: {:?}", err),
            }

            if Mux::get().unwrap().is_empty() && crate::frontend::activity::Activity::count() == 0 {
                info!("No more tabs; all done!");
                return Ok(());
            }
        }
    }

    fn spawn_new_window(
        &self,
        _fontconfig: &Rc<FontConfiguration>,
        _tab: &Rc<dyn Tab>,
        _window_id: WindowId,
    ) -> anyhow::Result<()> {
        Ok(())
    }
}
