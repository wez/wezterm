//! The connection to the GUI subsystem
use failure::Fallible;
use std::cell::RefCell;
use std::io::Error as IoError;
use std::ptr::null_mut;
use std::sync::Arc;
use winapi::um::winuser::*;

pub struct Connection {}

thread_local! {
    static CONN: RefCell<Option<Arc<Connection>>> = RefCell::new(None);
}

impl Connection {
    pub fn get() -> Option<Arc<Self>> {
        let mut res = None;
        CONN.with(|m| {
            if let Some(mux) = &*m.borrow() {
                res = Some(Arc::clone(mux));
            }
        });
        res
    }

    pub fn init() -> Fallible<Arc<Self>> {
        let conn = Arc::new(Self {});
        CONN.with(|m| *m.borrow_mut() = Some(Arc::clone(&conn)));
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
            let res = unsafe { GetMessageW(&mut msg, null_mut(), 0, 0) };
            if res == -1 {
                return Err(IoError::last_os_error().into());
            }
            if res == 0 {
                return Ok(());
            }

            unsafe {
                TranslateMessage(&mut msg);
                DispatchMessageW(&mut msg);
            }
        }
    }
}
