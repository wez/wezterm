//! The connection to the GUI subsystem
use super::{HWindow, WindowInner};
use crate::connection::ConnectionOps;
use crate::spawn::*;
use crate::tasks::{Task, Tasks};
use failure::Fallible;
use promise::BasicExecutor;
use std::cell::RefCell;
use std::collections::HashMap;
use std::convert::TryInto;
use std::ptr::null_mut;
use std::rc::Rc;
use winapi::shared::basetsd::UINT_PTR;
use winapi::shared::minwindef::*;
use winapi::shared::windef::*;
use winapi::um::winbase::INFINITE;
use winapi::um::winnt::HANDLE;
use winapi::um::winuser::*;

pub struct Connection {
    event_handle: HANDLE,
    pub(crate) windows: RefCell<HashMap<HWindow, Rc<RefCell<WindowInner>>>>,
    tasks: Tasks,
    timers: RefCell<HashMap<UINT_PTR, UINT_PTR>>,
}

impl ConnectionOps for Connection {
    fn terminate_message_loop(&self) {
        unsafe {
            PostQuitMessage(0);
        }
    }

    fn run_message_loop(&self) -> Fallible<()> {
        let mut msg: MSG = unsafe { std::mem::zeroed() };
        loop {
            SPAWN_QUEUE.run();

            let res = unsafe { PeekMessageW(&mut msg, null_mut(), 0, 0, PM_REMOVE) };
            if res != 0 {
                if msg.message == WM_QUIT {
                    // Clear our state before we exit, otherwise we can
                    // trigger `drop` handlers during shutdown and that
                    // can have bad interactions
                    self.windows.borrow_mut().clear();
                    return Ok(());
                }

                unsafe {
                    // We don't use TranslateMessage because we don't
                    // want to generate WM_CHAR internally
                    // TranslateMessage(&mut msg);
                    DispatchMessageW(&mut msg);
                }
            } else {
                self.wait_message();
            }
        }
    }

    fn spawn_task<F: std::future::Future<Output = ()> + 'static>(&self, future: F) {
        let id = self.tasks.add_task(Task(Box::pin(future)));
        Self::wake_task_by_id(id);
    }

    fn wake_task_by_id(slot: usize) {
        SpawnQueueExecutor {}.execute(Box::new(move || {
            let conn = Connection::get().unwrap();
            conn.tasks.poll_by_slot(slot);
        }));
    }

    fn schedule_timer<F: FnMut() + 'static>(&self, interval: std::time::Duration, callback: F) {
        let millis = interval
            .as_millis()
            .try_into()
            .expect("duration is too large to fit in 32 bits");
        let callback = Box::into_raw(Box::new(callback));

        extern "system" fn timer_proc<F: FnMut()>(
            _hwnd: HWND,
            _msg: UINT,
            timer_id: UINT_PTR,
            _lparam: DWORD,
        ) {
            let conn = Connection::get().unwrap();
            if let Some(cb) = conn.timers.borrow().get(&timer_id).map(|cb| *cb) {
                unsafe {
                    let callback: *mut F = cb as _;
                    (*callback)();
                }
            };
        }

        let timer_id = unsafe {
            SetTimer(
                std::ptr::null_mut(),
                callback as UINT_PTR,
                millis,
                Some(timer_proc::<F>),
            )
        };
        self.timers
            .borrow_mut()
            .insert(timer_id, callback as UINT_PTR);
    }
}

impl Connection {
    pub(crate) fn create_new() -> Fallible<Self> {
        let event_handle = SPAWN_QUEUE.event_handle.0;
        Ok(Self {
            event_handle,
            windows: RefCell::new(HashMap::new()),
            timers: RefCell::new(HashMap::new()),
            tasks: Default::default(),
        })
    }

    fn wait_message(&self) {
        unsafe {
            MsgWaitForMultipleObjects(1, &self.event_handle, 0, INFINITE, QS_ALLEVENTS);
        }
    }

    pub fn executor() -> impl BasicExecutor {
        SpawnQueueExecutor {}
    }

    pub fn low_pri_executor() -> impl BasicExecutor {
        LowPriSpawnQueueExecutor {}
    }

    fn get_window(&self, handle: HWindow) -> Option<Rc<RefCell<WindowInner>>> {
        self.windows.borrow().get(&handle).map(Rc::clone)
    }

    pub(crate) fn with_window_inner<R, F: FnMut(&mut WindowInner) -> Fallible<R> + Send + 'static>(
        window: HWindow,
        mut f: F,
    ) -> promise::Future<R>
    where
        R: Send + 'static,
    {
        let mut prom = promise::Promise::new();
        let future = prom.get_future().unwrap();
        SpawnQueueExecutor {}.execute(Box::new(move || {
            if let Some(handle) = Connection::get()
                .expect("Connection::init has not been called")
                .get_window(window)
            {
                let mut inner = handle.borrow_mut();
                prom.result(f(&mut inner));
            }
        }));

        future
    }
}
