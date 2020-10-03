use anyhow::Error;
use downcast_rs::{impl_downcast, Downcast};
use std::cell::RefCell;
use std::rc::Rc;

pub mod gui;

pub use config::FrontEndSelection;

thread_local! {
    static FRONT_END: RefCell<Option<Rc<dyn FrontEnd>>> = RefCell::new(None);
}

pub fn front_end() -> Option<Rc<dyn FrontEnd>> {
    let mut res = None;
    FRONT_END.with(|f| {
        if let Some(me) = &*f.borrow() {
            res = Some(Rc::clone(me));
        }
    });
    res
}

pub fn shutdown() {
    FRONT_END.with(|f| drop(f.borrow_mut().take()));
}

pub fn try_new(sel: FrontEndSelection) -> Result<Rc<dyn FrontEnd>, Error> {
    let front_end = match sel {
        FrontEndSelection::Software => gui::GuiFrontEnd::try_new_swrast(),
        FrontEndSelection::OldSoftware => gui::GuiFrontEnd::try_new_no_opengl(),
        FrontEndSelection::OpenGL => gui::GuiFrontEnd::try_new(),
    };

    let front_end = front_end?;

    FRONT_END.with(|f| *f.borrow_mut() = Some(Rc::clone(&front_end)));

    Ok(front_end)
}

pub trait FrontEnd: Downcast {
    /// Run the event loop.  Does not return until there is either a fatal
    /// error, or until there are no more windows left to manage.
    fn run_forever(&self) -> anyhow::Result<()>;
}
impl_downcast!(FrontEnd);
