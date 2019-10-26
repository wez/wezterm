use crate::config::Config;
use crate::font::FontConfiguration;
use crate::mux::tab::Tab;
use crate::mux::window::WindowId;
use crate::mux::Mux;
use downcast_rs::{impl_downcast, Downcast};
use failure::{format_err, Error, Fallible};
use lazy_static::lazy_static;
use promise::Executor;
use serde_derive::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

#[cfg(feature = "enable-winit")]
pub mod glium;
pub mod guicommon;
pub mod muxserver;
pub mod software;
#[cfg(all(unix, not(target_os = "macos")))]
pub mod xwindows;

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum FrontEndSelection {
    Glutin,
    X11,
    MuxServer,
    Null,
    Software,
    OpenGL,
}

impl Default for FrontEndSelection {
    fn default() -> Self {
        if cfg!(all(unix, not(target_os = "macos"))) {
            FrontEndSelection::X11
        } else if cfg!(feature = "enable-winit") {
            FrontEndSelection::Glutin
        } else {
            FrontEndSelection::OpenGL
        }
    }
}

lazy_static! {
    static ref EXECUTOR: Mutex<Option<Box<dyn Executor>>> = Mutex::new(None);
}
thread_local! {
    static FRONT_END: RefCell<Option<Rc<dyn FrontEnd>>> = RefCell::new(None);
}

pub fn gui_executor() -> Option<Box<dyn Executor>> {
    let locked = EXECUTOR.lock().unwrap();
    match locked.as_ref() {
        Some(exec) => Some(exec.clone_executor()),
        None => None,
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
    pub fn try_new(self, mux: &Rc<Mux>) -> Result<Rc<dyn FrontEnd>, Error> {
        let front_end = match self {
            #[cfg(feature = "enable-winit")]
            FrontEndSelection::Glutin => glium::glutinloop::GlutinFrontEnd::try_new(mux),
            #[cfg(not(feature = "enable-winit"))]
            FrontEndSelection::Glutin => failure::bail!("Glutin not compiled in"),
            #[cfg(all(unix, not(target_os = "macos")))]
            FrontEndSelection::X11 => xwindows::x11loop::X11FrontEnd::try_new(mux),
            #[cfg(not(all(unix, not(target_os = "macos"))))]
            FrontEndSelection::X11 => failure::bail!("X11 not compiled in"),
            FrontEndSelection::MuxServer => muxserver::MuxServerFrontEnd::try_new(mux),
            FrontEndSelection::Null => muxserver::MuxServerFrontEnd::new_null(mux),
            FrontEndSelection::Software => software::SoftwareFrontEnd::try_new_no_opengl(mux),
            FrontEndSelection::OpenGL => software::SoftwareFrontEnd::try_new(mux),
        }?;

        EXECUTOR.lock().unwrap().replace(front_end.gui_executor());
        FRONT_END.with(|f| *f.borrow_mut() = Some(Rc::clone(&front_end)));

        Ok(front_end)
    }

    // TODO: find or build a proc macro for this
    pub fn variants() -> Vec<&'static str> {
        vec!["Glutin", "X11", "MuxServer", "Null", "Software", "OpenGL"]
    }
}

impl std::str::FromStr for FrontEndSelection {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_ref() {
            "glutin" => Ok(FrontEndSelection::Glutin),
            "x11" => Ok(FrontEndSelection::X11),
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
        config: &Arc<Config>,
        fontconfig: &Rc<FontConfiguration>,
        tab: &Rc<dyn Tab>,
        window_id: WindowId,
    ) -> Fallible<()>;

    fn gui_executor(&self) -> Box<dyn Executor>;
}
impl_downcast!(FrontEnd);
