use crate::font::FontConfiguration;
use crate::mux::tab::Tab;
use crate::mux::window::WindowId;
use anyhow::{anyhow, Error};
use downcast_rs::{impl_downcast, Downcast};
use luahelper::impl_lua_conversion;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};

pub mod activity;
pub mod gui;
pub mod muxserver;

#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
pub enum FrontEndSelection {
    OpenGL,
    Software,
    OldSoftware,
    MuxServer,
    Null,
}
impl_lua_conversion!(FrontEndSelection);

impl Default for FrontEndSelection {
    fn default() -> Self {
        FrontEndSelection::OpenGL
    }
}

thread_local! {
    static FRONT_END: RefCell<Option<Rc<dyn FrontEnd>>> = RefCell::new(None);
}

static HAS_GUI_FRONT_END: AtomicBool = AtomicBool::new(false);

/// Returns true if a GUI frontend has been initialized, which implies that
/// it makes sense (and is safe) to use the window crate and associated
/// functionality
pub fn has_gui_front_end() -> bool {
    HAS_GUI_FRONT_END.load(Ordering::Acquire)
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

impl FrontEndSelection {
    pub fn try_new(self) -> Result<Rc<dyn FrontEnd>, Error> {
        let (front_end, is_gui) = match self {
            FrontEndSelection::MuxServer => (muxserver::MuxServerFrontEnd::try_new(), false),
            FrontEndSelection::Null => (muxserver::MuxServerFrontEnd::new_null(), false),
            FrontEndSelection::Software => (gui::GuiFrontEnd::try_new_swrast(), true),
            FrontEndSelection::OldSoftware => (gui::GuiFrontEnd::try_new_no_opengl(), true),
            FrontEndSelection::OpenGL => (gui::GuiFrontEnd::try_new(), true),
        };

        let front_end = front_end?;

        FRONT_END.with(|f| *f.borrow_mut() = Some(Rc::clone(&front_end)));
        HAS_GUI_FRONT_END.store(is_gui, Ordering::Release);

        Ok(front_end)
    }

    // TODO: find or build a proc macro for this
    pub fn variants() -> Vec<&'static str> {
        vec!["OpenGL", "Software", "OldSoftware", "MuxServer", "Null"]
    }
}

impl std::str::FromStr for FrontEndSelection {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_ref() {
            "muxserver" => Ok(FrontEndSelection::MuxServer),
            "null" => Ok(FrontEndSelection::Null),
            "software" => Ok(FrontEndSelection::Software),
            "oldsoftware" => Ok(FrontEndSelection::OldSoftware),
            "opengl" => Ok(FrontEndSelection::OpenGL),
            _ => Err(anyhow!(
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
    fn run_forever(&self) -> anyhow::Result<()>;

    fn spawn_new_window(
        &self,
        fontconfig: &Rc<FontConfiguration>,
        tab: &Rc<Tab>,
        window_id: WindowId,
    ) -> anyhow::Result<()>;
}
impl_downcast!(FrontEnd);
