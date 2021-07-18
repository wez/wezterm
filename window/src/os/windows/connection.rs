//! The connection to the GUI subsystem
use super::{HWindow, WindowInner};
use crate::connection::ConnectionOps;
use crate::spawn::*;
use crate::Appearance;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ptr::null_mut;
use std::rc::Rc;
use winapi::um::winbase::INFINITE;
use winapi::um::winnt::HANDLE;
use winapi::um::winuser::*;
use winreg::{enums::HKEY_CURRENT_USER, RegKey};

pub struct Connection {
    event_handle: HANDLE,
    pub(crate) windows: RefCell<HashMap<HWindow, Rc<RefCell<WindowInner>>>>,
    pub(crate) gl_connection: RefCell<Option<Rc<crate::egl::GlConnection>>>,
}

pub(crate) fn get_appearance() -> Appearance {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    match hkcu.open_subkey("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize") {
        Ok(theme) => {
            let light = theme.get_value::<u32, _>("AppsUseLightTheme").unwrap_or(1) == 1;
            if light {
                Appearance::Light
            } else {
                Appearance::Dark
            }
        }
        _ => Appearance::Light,
    }
}

impl ConnectionOps for Connection {
    fn terminate_message_loop(&self) {
        unsafe {
            PostQuitMessage(0);
        }
    }

    fn get_appearance(&self) -> Appearance {
        get_appearance()
    }

    fn run_message_loop(&self) -> anyhow::Result<()> {
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
                    // We don't want to call TranslateMessage here
                    // unconditionally.  Instead, we perform translation
                    // in a handful of special cases in window.rs.
                    DispatchMessageW(&mut msg);
                }
            } else {
                self.wait_message();
            }
        }
    }
}

impl Connection {
    pub(crate) fn create_new() -> anyhow::Result<Self> {
        let event_handle = SPAWN_QUEUE.event_handle.0;
        Ok(Self {
            event_handle,
            windows: RefCell::new(HashMap::new()),
            gl_connection: RefCell::new(None),
        })
    }

    fn wait_message(&self) {
        unsafe {
            MsgWaitForMultipleObjects(1, &self.event_handle, 0, INFINITE, QS_ALLEVENTS);
        }
    }

    pub(crate) fn get_window(&self, handle: HWindow) -> Option<Rc<RefCell<WindowInner>>> {
        self.windows.borrow().get(&handle).map(Rc::clone)
    }

    pub(crate) fn with_window_inner<
        R,
        F: FnOnce(&mut WindowInner) -> anyhow::Result<R> + Send + 'static,
    >(
        window: HWindow,
        f: F,
    ) -> promise::Future<R>
    where
        R: Send + 'static,
    {
        let mut prom = promise::Promise::new();
        let future = prom.get_future().unwrap();
        promise::spawn::spawn_into_main_thread(async move {
            if let Some(handle) = Connection::get()
                .expect("Connection::init has not been called")
                .get_window(window)
            {
                let mut inner = handle.borrow_mut();
                prom.result(f(&mut inner));
            }
        })
        .detach();

        future
    }
}
