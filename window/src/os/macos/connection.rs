// let () = msg_send! is a common pattern for objc
#![allow(clippy::let_unit_value)]

use super::window::WindowInner;
use crate::connection::ConnectionOps;
use crate::spawn::*;
use cocoa::appkit::{NSApp, NSApplication, NSApplicationActivationPolicyRegular};
use cocoa::base::{id, nil};
use core_foundation::date::CFAbsoluteTimeGetCurrent;
use core_foundation::runloop::*;
use objc::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::AtomicUsize;

pub struct Connection {
    ns_app: id,
    pub(crate) windows: RefCell<HashMap<usize, Rc<RefCell<WindowInner>>>>,
    pub(crate) next_window_id: AtomicUsize,
    #[cfg(feature = "opengl")]
    pub(crate) gl_connection: RefCell<Option<Rc<crate::egl::GlConnection>>>,
}

impl Connection {
    pub(crate) fn create_new() -> anyhow::Result<Self> {
        // Ensure that the SPAWN_QUEUE is created; it will have nothing
        // to run right now.
        SPAWN_QUEUE.run();

        unsafe {
            let ns_app = NSApp();
            ns_app.setActivationPolicy_(NSApplicationActivationPolicyRegular);
            let conn = Self {
                ns_app,
                windows: RefCell::new(HashMap::new()),
                next_window_id: AtomicUsize::new(1),
                #[cfg(feature = "opengl")]
                gl_connection: RefCell::new(None),
            };
            Ok(conn)
        }
    }

    pub(crate) fn next_window_id(&self) -> usize {
        self.next_window_id
            .fetch_add(1, ::std::sync::atomic::Ordering::Relaxed)
    }

    pub(crate) fn window_by_id(&self, window_id: usize) -> Option<Rc<RefCell<WindowInner>>> {
        self.windows.borrow().get(&window_id).map(Rc::clone)
    }

    pub(crate) fn with_window_inner<
        R,
        F: FnMut(&mut WindowInner) -> anyhow::Result<R> + Send + 'static,
    >(
        window_id: usize,
        mut f: F,
    ) -> promise::Future<R>
    where
        R: Send + 'static,
    {
        let mut prom = promise::Promise::new();
        let future = prom.get_future().unwrap();
        promise::spawn::spawn_into_main_thread(async move {
            if let Some(handle) = Connection::get().unwrap().window_by_id(window_id) {
                let mut inner = handle.borrow_mut();
                prom.result(f(&mut inner));
            }
        })
        .detach();

        future
    }
}

impl ConnectionOps for Connection {
    fn terminate_message_loop(&self) {
        unsafe {
            let () = msg_send![NSApp(), stop: nil];
        }
    }

    fn run_message_loop(&self) -> anyhow::Result<()> {
        unsafe {
            self.ns_app.run();
        }
        self.windows.borrow_mut().clear();
        Ok(())
    }

    fn hide_application(&self) {
        unsafe {
            let () = msg_send![self.ns_app, hide: self.ns_app];
        }
    }

    fn schedule_timer<F: FnMut() + 'static>(&self, interval: std::time::Duration, callback: F) {
        let secs_f64 =
            (interval.as_secs() as f64) + (f64::from(interval.subsec_nanos()) / 1_000_000_000_f64);

        let callback = Box::into_raw(Box::new(callback));

        extern "C" fn timer_callback<F: FnMut()>(
            _timer_ref: *mut __CFRunLoopTimer,
            callback_ptr: *mut std::ffi::c_void,
        ) {
            unsafe {
                let callback: *mut F = callback_ptr as _;
                (*callback)();
            }
        }

        extern "C" fn release_callback<F: FnMut()>(info: *const std::ffi::c_void) {
            let callback: Box<F> = unsafe { Box::from_raw(info as *mut F) };
            drop(callback);
        }

        let timer_ref = unsafe {
            CFRunLoopTimerCreate(
                std::ptr::null(),
                CFAbsoluteTimeGetCurrent() + secs_f64,
                secs_f64,
                0,
                0,
                timer_callback::<F>,
                &mut CFRunLoopTimerContext {
                    copyDescription: None,
                    info: callback as _,
                    release: Some(release_callback::<F>),
                    retain: None,
                    version: 0,
                },
            )
        };

        unsafe {
            CFRunLoopAddTimer(CFRunLoopGetCurrent(), timer_ref, kCFRunLoopCommonModes);
        }
    }
}
