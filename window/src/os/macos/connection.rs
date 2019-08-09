use cocoa::appkit::{NSApp, NSApplication, NSApplicationActivationPolicyRegular};
use cocoa::base::{id, nil};
use failure::Fallible;
use objc::*;
use std::cell::RefCell;
use std::sync::Arc;

pub struct Connection {
    ns_app: id,
}

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
        unsafe {
            let ns_app = NSApp();
            ns_app.setActivationPolicy_(NSApplicationActivationPolicyRegular);
            let conn = Arc::new(Self { ns_app });
            CONN.with(|m| *m.borrow_mut() = Some(Arc::clone(&conn)));
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
}
