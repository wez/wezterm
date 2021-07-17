use crate::{Appearance, Connection};
use anyhow::Result as Fallible;
use std::cell::RefCell;
use std::rc::Rc;

thread_local! {
    static CONN: RefCell<Option<Rc<Connection>>> = RefCell::new(None);
}

pub fn shutdown() {
    CONN.with(|m| drop(m.borrow_mut().take()));
}

pub trait ConnectionOps {
    fn get() -> Option<Rc<Connection>> {
        let mut res = None;
        CONN.with(|m| {
            if let Some(mux) = &*m.borrow() {
                res = Some(Rc::clone(mux));
            }
        });
        res
    }

    fn default_dpi(&self) -> f64 {
        crate::DEFAULT_DPI
    }

    fn init() -> Fallible<Rc<Connection>> {
        let conn = Rc::new(Connection::create_new()?);
        CONN.with(|m| *m.borrow_mut() = Some(Rc::clone(&conn)));
        crate::spawn::SPAWN_QUEUE.register_promise_schedulers();
        Ok(conn)
    }

    fn terminate_message_loop(&self);
    fn run_message_loop(&self) -> Fallible<()>;

    /// Retrieve the current appearance for the application.
    fn get_appearance(&self) -> Appearance {
        Appearance::Light
    }

    /// Hide the application.
    /// This actions hides all of the windows of the application and switches
    /// focus away from it.
    fn hide_application(&self) {}
}
