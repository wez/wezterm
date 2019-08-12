//! The connection to the GUI subsystem
use super::EventHandle;
use super::{HWindow, WindowInner};
use failure::Fallible;
use promise::{BasicExecutor, SpawnFunc};
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::ptr::null_mut;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use winapi::um::winbase::INFINITE;
use winapi::um::winnt::HANDLE;
use winapi::um::winuser::*;

lazy_static::lazy_static! {
    static ref SPAWN_QUEUE: Arc<SpawnQueue> = Arc::new(SpawnQueue::new().expect("failed to create SpawnQueue"));
}

thread_local! {
    static CONN: RefCell<Option<Rc<Connection>>> = RefCell::new(None);
}

struct SpawnQueue {
    spawned_funcs: Mutex<VecDeque<SpawnFunc>>,
    event_handle: EventHandle,
}

impl SpawnQueue {
    fn new() -> Fallible<Self> {
        let spawned_funcs = Mutex::new(VecDeque::new());
        let event_handle = EventHandle::new_manual_reset().expect("EventHandle creation failed");
        Ok(Self {
            spawned_funcs,
            event_handle,
        })
    }

    fn spawn(&self, f: SpawnFunc) {
        self.spawned_funcs.lock().unwrap().push_back(f);
        self.event_handle.set_event();
    }

    fn run(&self) {
        self.event_handle.reset_event();
        loop {
            if let Some(func) = self.spawned_funcs.lock().unwrap().pop_front() {
                func();
            } else {
                return;
            }
        }
    }
}

pub struct Connection {
    event_handle: HANDLE,
    pub(crate) windows: Mutex<HashMap<HWindow, Rc<RefCell<WindowInner>>>>,
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

    fn new() -> Self {
        let event_handle = SPAWN_QUEUE.event_handle.0;
        Self {
            event_handle,
            windows: Mutex::new(HashMap::new()),
        }
    }

    pub fn init() -> Fallible<Rc<Self>> {
        let conn = Rc::new(Self::new());
        CONN.with(|m| *m.borrow_mut() = Some(Rc::clone(&conn)));
        Ok(conn)
    }

    pub fn terminate_message_loop(&self) {
        unsafe {
            PostQuitMessage(0);
        }
    }

    pub fn run_message_loop(&self) -> Fallible<()> {
        let mut msg: MSG = unsafe { std::mem::zeroed() };
        loop {
            SPAWN_QUEUE.run();

            let res = unsafe { PeekMessageW(&mut msg, null_mut(), 0, 0, PM_REMOVE) };
            if res != 0 {
                if msg.message == WM_QUIT {
                    // Clear our state before we exit, otherwise we can
                    // trigger `drop` handlers during shutdown and that
                    // can have bad interactions
                    self.windows.lock().unwrap().clear();
                    return Ok(());
                }

                unsafe {
                    TranslateMessage(&mut msg);
                    DispatchMessageW(&mut msg);
                }
            } else {
                self.wait_message();
            }
        }
    }

    fn wait_message(&self) {
        unsafe {
            MsgWaitForMultipleObjects(1, &self.event_handle, 0, INFINITE, QS_ALLEVENTS);
        }
    }

    pub fn executor() -> impl BasicExecutor {
        SpawnQueueExecutor {}
    }

    fn get_window(&self, handle: HWindow) -> Option<Rc<RefCell<WindowInner>>> {
        self.windows.lock().unwrap().get(&handle).map(Rc::clone)
    }

    pub(crate) fn with_window_inner<F: FnMut(&mut WindowInner) + Send + 'static>(
        window: HWindow,
        mut f: F,
    ) {
        SpawnQueueExecutor {}.execute(Box::new(move || {
            if let Some(handle) = Connection::get()
                .expect("Connection::init has not been called")
                .get_window(window)
            {
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
