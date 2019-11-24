use crate::font::FontConfiguration;
use crate::mux::tab::Tab;
use crate::mux::window::WindowId;
use downcast_rs::{impl_downcast, Downcast};
use failure::{format_err, Error, Fallible};
use lazy_static::lazy_static;
use promise::Executor;
use serde_derive::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Mutex;

pub mod gui;
pub mod muxserver;

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum FrontEndSelection {
    OpenGL,
    Software,
    MuxServer,
    Null,
}

impl Default for FrontEndSelection {
    fn default() -> Self {
        FrontEndSelection::OpenGL
    }
}

lazy_static! {
    static ref EXECUTOR: Mutex<Option<Box<dyn Executor>>> = Mutex::new(None);
    static ref LOW_PRI_EXECUTOR: Mutex<Option<Box<dyn Executor>>> = Mutex::new(None);
}
thread_local! {
    static FRONT_END: RefCell<Option<Rc<dyn FrontEnd>>> = RefCell::new(None);
}

pub fn executor() -> Box<dyn Executor> {
    let locked = EXECUTOR.lock().unwrap();
    match locked.as_ref() {
        Some(exec) => exec.clone_executor(),
        None => panic!("executor machinery not yet configured"),
    }
}

pub fn low_pri_executor() -> Box<dyn Executor> {
    let locked = LOW_PRI_EXECUTOR.lock().unwrap();
    match locked.as_ref() {
        Some(exec) => exec.clone_executor(),
        None => panic!("executor machinery not yet configured"),
    }
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

impl FrontEndSelection {
    pub fn try_new(self) -> Result<Rc<dyn FrontEnd>, Error> {
        let front_end = match self {
            FrontEndSelection::MuxServer => muxserver::MuxServerFrontEnd::try_new(),
            FrontEndSelection::Null => muxserver::MuxServerFrontEnd::new_null(),
            FrontEndSelection::Software => gui::GuiFrontEnd::try_new_no_opengl(),
            FrontEndSelection::OpenGL => gui::GuiFrontEnd::try_new(),
        }?;

        EXECUTOR.lock().unwrap().replace(front_end.executor());
        LOW_PRI_EXECUTOR
            .lock()
            .unwrap()
            .replace(front_end.low_pri_executor());
        FRONT_END.with(|f| *f.borrow_mut() = Some(Rc::clone(&front_end)));

        Ok(front_end)
    }

    // TODO: find or build a proc macro for this
    pub fn variants() -> Vec<&'static str> {
        vec!["OpenGL", "Software", "MuxServer", "Null"]
    }
}

impl std::str::FromStr for FrontEndSelection {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_ref() {
            "muxserver" => Ok(FrontEndSelection::MuxServer),
            "null" => Ok(FrontEndSelection::Null),
            "software" => Ok(FrontEndSelection::Software),
            "opengl" => Ok(FrontEndSelection::OpenGL),
            _ => Err(format_err!(
                "{} is not a valid FrontEndSelection variant, possible values are {:?}",
                s,
                FrontEndSelection::variants()
            )),
        }
    }
}

pub trait FrontEnd: Downcast {
    /// Run the event loop.  Does not return until there is either a fatal
    /// error, or until there are no more windows left to manage.
    fn run_forever(&self) -> Result<(), Error>;

    fn spawn_new_window(
        &self,
        fontconfig: &Rc<FontConfiguration>,
        tab: &Rc<dyn Tab>,
        window_id: WindowId,
    ) -> Fallible<()>;

    fn executor(&self) -> Box<dyn Executor>;
    fn low_pri_executor(&self) -> Box<dyn Executor>;
}
impl_downcast!(FrontEnd);
