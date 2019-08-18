use crate::Connection;
use std::cell::{Cell, RefCell};
use std::future::Future;
use std::task::{Context, RawWaker, RawWakerVTable, Waker};

pub struct Task(pub std::pin::Pin<Box<dyn Future<Output = ()>>>);

enum Slot {
    Vacant { next_vacant: usize },
    Running(Option<Task>),
}

#[derive(Default)]
pub struct Tasks {
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
        let raw = RawWaker::new(unsafe { std::mem::transmute(slot) }, &VTBL);
        unsafe { Waker::from_raw(raw) }
    }

    unsafe fn waker_clone(p: *const ()) -> RawWaker {
        RawWaker::new(p, &VTBL)
    }

    unsafe fn waker_wake(p: *const ()) {
        let id: usize = std::mem::transmute(p);
        Connection::wake_task_by_id(id);
    }

    unsafe fn waker_wake_by_ref(p: *const ()) {
        let id: usize = std::mem::transmute(p);
        Connection::wake_task_by_id(id);
    }

    unsafe fn waker_drop(p: *const ()) {
        /* no action required */
    }
}
