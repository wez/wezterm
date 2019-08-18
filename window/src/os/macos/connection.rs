use super::window::WindowInner;
use crate::connection::ConnectionOps;
use crate::tasks::{Task, Tasks};
use cocoa::appkit::{NSApp, NSApplication, NSApplicationActivationPolicyRegular};
use cocoa::base::{id, nil};
use core_foundation::runloop::*;
use failure::Fallible;
use objc::*;
use promise::{BasicExecutor, SpawnFunc};
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Mutex};

pub struct Connection {
    ns_app: id,
    pub(crate) windows: RefCell<HashMap<usize, Rc<RefCell<WindowInner>>>>,
    pub(crate) next_window_id: AtomicUsize,
    tasks: Tasks,
}

struct SpawnQueue {
    spawned_funcs: Mutex<VecDeque<SpawnFunc>>,
}

lazy_static::lazy_static! {
    static ref SPAWN_QUEUE: Arc<SpawnQueue> = Arc::new(SpawnQueue::new().expect("failed to create SpawnQueue"));
}

impl Drop for SpawnQueue {
    fn drop(&mut self) {}
}

impl SpawnQueue {
    fn new() -> Fallible<Self> {
        let spawned_funcs = Mutex::new(VecDeque::new());

        let observer = unsafe {
            CFRunLoopObserverCreate(
                std::ptr::null(),
                kCFRunLoopAllActivities,
                1,
                0,
                SpawnQueue::trigger,
                std::ptr::null_mut(),
            )
        };
        unsafe {
            CFRunLoopAddObserver(CFRunLoopGetMain(), observer, kCFRunLoopCommonModes);
        }

        Ok(Self { spawned_funcs })
    }

    extern "C" fn trigger(_observer: *mut __CFRunLoopObserver, _: u32, _: *mut std::ffi::c_void) {
        SPAWN_QUEUE.run();
    }

    fn spawn(&self, f: SpawnFunc) {
        self.spawned_funcs.lock().unwrap().push_back(f);
        unsafe {
            CFRunLoopWakeUp(CFRunLoopGetMain());
        }
    }

    // This needs to be a separate function from the loop in `run`
    // in order for the lock to be released before we call the
    // returned function
    fn pop_func(&self) -> Option<SpawnFunc> {
        self.spawned_funcs.lock().unwrap().pop_front()
    }

    fn run(&self) {
        while let Some(func) = self.pop_func() {
            func();
        }
    }
}

impl Connection {
    pub(crate) fn create_new() -> Fallible<Self> {
        // Ensure that the SPAWN_QUEUE is created; it will have nothing
        // to run right now.
        SPAWN_QUEUE.run();

        unsafe {
            let ns_app = NSApp();
            ns_app.setActivationPolicy_(NSApplicationActivationPolicyRegular);
            let conn = Self {
                ns_app,
                windows: RefCell::new(HashMap::new()),
                tasks: Default::default(),
                next_window_id: AtomicUsize::new(1),
            };
            Ok(conn)
        }
    }

    pub(crate) fn next_window_id(&self) -> usize {
        self.next_window_id
            .fetch_add(1, ::std::sync::atomic::Ordering::Relaxed)
    }

    fn window_by_id(&self, window_id: usize) -> Option<Rc<RefCell<WindowInner>>> {
        self.windows.borrow().get(&window_id).map(Rc::clone)
    }

    pub(crate) fn with_window_inner<F: FnMut(&mut WindowInner) + Send + 'static>(
        window_id: usize,
        mut f: F,
    ) {
        SpawnQueueExecutor {}.execute(Box::new(move || {
            if let Some(handle) = Connection::get().unwrap().window_by_id(window_id) {
                let mut inner = handle.borrow_mut();
                f(&mut inner);
            }
        }));
    }

    pub(crate) fn wake_task_by_id(slot: usize) {
        SpawnQueueExecutor {}.execute(Box::new(move || {
            let conn = Connection::get().unwrap();
            conn.tasks.poll_by_slot(slot);
        }));
    }
}

impl ConnectionOps for Connection {
    fn terminate_message_loop(&self) {
        unsafe {
            let () = msg_send![NSApp(), stop: nil];
        }
    }

    fn run_message_loop(&self) -> Fallible<()> {
        unsafe {
            self.ns_app.run();
        }
        self.windows.borrow_mut().clear();
        Ok(())
    }

    fn spawn_task<F: std::future::Future<Output = ()> + 'static>(&self, future: F) {
        let id = self.tasks.add_task(Task(Box::pin(future)));
        Self::wake_task_by_id(id);
    }
}

struct SpawnQueueExecutor;
impl BasicExecutor for SpawnQueueExecutor {
    fn execute(&self, f: SpawnFunc) {
        SPAWN_QUEUE.spawn(f)
    }
}
