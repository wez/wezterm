use super::ExitStatus;
use crate::config::Config;
use crate::font::FontConfiguration;
use crate::guicommon::tabs::Tab;
use crate::mux::Mux;
use failure::Error;
use promise::Executor;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

#[derive(Debug, Deserialize, Clone, Copy)]
pub enum GuiSelection {
    Glutin,
    X11,
}

impl Default for GuiSelection {
    fn default() -> Self {
        if cfg!(feature = "force-glutin") {
            GuiSelection::Glutin
        } else if cfg!(all(unix, not(target_os = "macos"))) {
            GuiSelection::X11
        } else {
            GuiSelection::Glutin
        }
    }
}

thread_local! {
    static GUI_SYSTEM: RefCell<Option<Rc<GuiSystem>>> = RefCell::new(None);
}

impl GuiSelection {
    pub fn try_new(self, mux: &Rc<Mux>) -> Result<Rc<GuiSystem>, Error> {
        let system = match self {
            GuiSelection::Glutin => glutinloop::GlutinGuiSystem::try_new(mux),
            #[cfg(all(unix, not(target_os = "macos")))]
            GuiSelection::X11 => x11::X11GuiSystem::try_new(mux),
            #[cfg(not(all(unix, not(target_os = "macos"))))]
            GuiSelection::X11 => bail!("X11 not compiled in"),
        };

        if let Ok(sys) = &system {
            GUI_SYSTEM.with(|g| {
                *g.borrow_mut() = Some(Rc::clone(&sys));
            });
        }

        system
    }

    /// Returns a reference to the gui system.
    /// Will return None if not called on the gui thread.
    pub fn get() -> Option<Rc<GuiSystem>> {
        let mut res = None;
        GUI_SYSTEM.with(|g| {
            if let Some(sys) = &*g.borrow() {
                res = Some(Rc::clone(sys));
            }
        });
        res
    }

    // TODO: find or build a proc macro for this
    pub fn variants() -> Vec<&'static str> {
        vec!["Glutin", "X11"]
    }
}

impl std::str::FromStr for GuiSelection {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_ref() {
            "glutin" => Ok(GuiSelection::Glutin),
            "x11" => Ok(GuiSelection::X11),
            _ => Err(format_err!(
                "{} is not a valid GuiSelection variant, possible values are {:?}",
                s,
                GuiSelection::variants()
            )),
        }
    }
}

pub trait GuiSystem {
    /// Run the event loop.  Does not return until there is either a fatal
    /// error, or until there are no more windows left to manage.
    fn run_forever(&self) -> Result<(), Error>;

    fn spawn_new_window(
        &self,
        config: &Arc<Config>,
        fontconfig: &Rc<FontConfiguration>,
        tab: &Rc<Tab>,
    ) -> Result<(), Error>;

    fn gui_executor(&self) -> Box<Executor>;
}

pub mod glutinloop;
#[cfg(all(unix, not(feature = "force-glutin"), not(target_os = "macos")))]
pub mod x11;

#[derive(Debug, Fail)]
#[allow(dead_code)]
pub enum SessionTerminated {
    #[fail(display = "Process exited: {:?}", status)]
    ProcessStatus { status: ExitStatus },
    #[fail(display = "Error: {:?}", err)]
    Error { err: Error },
    #[fail(display = "Window Closed")]
    WindowClosed,
}
