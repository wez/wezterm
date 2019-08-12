use super::window::WindowInner;
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
}

struct SpawnQueue {
    spawned_funcs: Mutex<VecDeque<SpawnFunc>>,
}

thread_local! {
    static CONN: RefCell<Option<Rc<Connection>>> = RefCell::new(None);
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

    fn run(&self) {
        loop {
            if let Some(func) = self.spawned_funcs.lock().unwrap().pop_front() {
                func();
            } else {
                return;
            }
        }
    }
}

impl Connection {
    pub fn get() -> Option<Rc<Self>> {
        let mut res = None;
        CONN.with(|m| {
            if let Some(mux) = &*m.borrow() {
                res = Some(Rc::clone(mux));
            }
        });
        res
    }

    pub fn init() -> Fallible<Rc<Self>> {
        // Ensure that the SPAWN_QUEUE is created; it will have nothing
        // to run right now.
        SPAWN_QUEUE.run();

        unsafe {
            let ns_app = NSApp();
            ns_app.setActivationPolicy_(NSApplicationActivationPolicyRegular);
            let conn = Rc::new(Self {
                ns_app,
                windows: RefCell::new(HashMap::new()),
                next_window_id: AtomicUsize::new(1),
            });
            CONN.with(|m| *m.borrow_mut() = Some(Rc::clone(&conn)));
            Ok(conn)
        }
    }

    pub fn terminate_message_loop(&self) {
        unsafe {
            let () = msg_send![NSApp(), stop: nil];
        }
    }

    pub fn run_message_loop(&self) -> Fallible<()> {
        unsafe {
            self.ns_app.run();
        }
        Ok(())
    }

    pub fn executor() -> impl BasicExecutor {
        SpawnQueueExecutor {}
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
}

struct SpawnQueueExecutor;
impl BasicExecutor for SpawnQueueExecutor {
    fn execute(&self, f: SpawnFunc) {
        SPAWN_QUEUE.spawn(f)
    }
}
