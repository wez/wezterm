use super::ExitStatus;
use crate::config::Config;
use crate::font::FontConfiguration;
use crate::{Child, MasterPty};
use failure::Error;
use std::rc::Rc;

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

impl GuiSelection {
    pub fn new(&self) -> Result<Rc<GuiSystem>, Error> {
        match self {
            GuiSelection::Glutin => glutinloop::GlutinGuiSystem::new(),
            GuiSelection::X11 => {
                #[cfg(all(unix, not(target_os = "macos")))]
                return x11::X11GuiSystem::new();
                #[cfg(not(all(unix, not(target_os = "macos"))))]
                bail!("X11 not compiled in");
            }
        }
    }
}

pub trait GuiSystem {
    /// Run the event loop.  Does not return until there is either a fatal
    /// error, or until there are no more windows left to manage.
    fn run_forever(
        &self,
        config: &Rc<Config>,
        fontconfig: &Rc<FontConfiguration>,
    ) -> Result<(), Error>;

    fn spawn_new_window(
        &self,
        terminal: term::Terminal,
        master: MasterPty,
        child: Child,
        config: &Rc<Config>,
        fontconfig: &Rc<FontConfiguration>,
    ) -> Result<(), Error>;
}

pub mod glutinloop;
#[cfg(all(unix, not(feature = "force-glutin"), not(target_os = "macos")))]
pub mod x11;

#[derive(Debug, Fail)]
pub enum SessionTerminated {
    #[fail(display = "Process exited: {:?}", status)]
    ProcessStatus { status: ExitStatus },
    #[fail(display = "Error: {:?}", err)]
    Error { err: Error },
    #[fail(display = "Window Closed")]
    WindowClosed,
}
