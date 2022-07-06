//! The connection to the GUI subsystem
use super::{HWindow, WindowInner};
use crate::connection::ConnectionOps;
use crate::screen::{ScreenInfo, Screens};
use crate::spawn::*;
use crate::{Appearance, ScreenRect};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::ptr::null_mut;
use std::rc::Rc;
use winapi::shared::minwindef::*;
use winapi::shared::windef::*;
use winapi::um::winbase::INFINITE;
use winapi::um::winnt::HANDLE;
use winapi::um::winuser::*;
use winreg::enums::HKEY_CURRENT_USER;
use winreg::RegKey;

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

    fn beep(&self) {
        unsafe {
            MessageBeep(MB_OK);
        }
    }

    fn screens(&self) -> anyhow::Result<Screens> {
        // Iterate the monitors.
        // The device names are things like "\\.\DISPLAY1" which isn't super
        // user friendly.  There may be an alternative API to get a better name,
        // but for now this is good enough.
        struct Info {
            primary: Option<ScreenInfo>,
            by_name: HashMap<String, ScreenInfo>,
            virtual_rect: ScreenRect,
        }

        unsafe extern "system" fn callback(
            mon: HMONITOR,
            _hdc: HDC,
            _rect: *mut RECT,
            data: LPARAM,
        ) -> i32 {
            let info: &mut Info = &mut *(data as *mut Info);
            let mut mi: MONITORINFOEXW = std::mem::zeroed();
            mi.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
            GetMonitorInfoW(mon, &mut mi as *mut MONITORINFOEXW as *mut MONITORINFO);

            // "\\.\DISPLAY1" -> "DISPLAY1"
            let len = mi.szDevice.iter().position(|&c| c == 0).unwrap_or(0);
            let monitor_name = OsString::from_wide(&mi.szDevice[0..len])
                .to_string_lossy()
                .to_string();
            let monitor_name = if let Some(name) = monitor_name.strip_prefix("\\\\.\\") {
                name.to_string()
            } else {
                monitor_name
            };

            let screen_info = ScreenInfo {
                name: monitor_name.clone(),
                rect: euclid::rect(
                    mi.rcMonitor.left as isize,
                    mi.rcMonitor.top as isize,
                    mi.rcMonitor.right as isize - mi.rcMonitor.left as isize,
                    mi.rcMonitor.bottom as isize - mi.rcMonitor.top as isize,
                ),
            };

            info.virtual_rect = info.virtual_rect.union(&screen_info.rect);

            if mi.dwFlags & MONITORINFOF_PRIMARY == MONITORINFOF_PRIMARY {
                info.primary.replace(screen_info.clone());
            }

            info.by_name.insert(monitor_name, screen_info);

            winapi::shared::ntdef::TRUE.into()
        }

        let mut info = Info {
            primary: None,
            by_name: HashMap::new(),
            virtual_rect: euclid::rect(0, 0, 0, 0),
        };
        unsafe {
            EnumDisplayMonitors(
                std::ptr::null_mut(),
                std::ptr::null(),
                Some(callback),
                &mut info as *mut _ as LPARAM,
            );
        }

        let main = info
            .primary
            .ok_or_else(|| anyhow::anyhow!("There is no primary monitor configured!?"))?;
        Ok(Screens {
            main,
            by_name: info.by_name,
            virtual_rect: info.virtual_rect,
        })
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
            MsgWaitForMultipleObjects(
                1,
                &self.event_handle,
                0,
                INFINITE,
                QS_ALLEVENTS | QS_ALLINPUT | QS_ALLPOSTMESSAGE,
            );
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
