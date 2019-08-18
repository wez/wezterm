use crate::Connection;
use failure::Fallible;
use std::cell::RefCell;
use std::rc::Rc;

thread_local! {
    static CONN: RefCell<Option<Rc<Connection>>> = RefCell::new(None);
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

    fn init() -> Fallible<Rc<Connection>> {
        let conn = Rc::new(Connection::create_new()?);
        CONN.with(|m| *m.borrow_mut() = Some(Rc::clone(&conn)));
        Ok(conn)
    }

    fn terminate_message_loop(&self);
    fn run_message_loop(&self) -> Fallible<()>;
    fn spawn_task<F: std::future::Future<Output = ()> + 'static>(&self, future: F);
    fn wake_task_by_id(slot: usize);
}
