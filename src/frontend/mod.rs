use crate::mux::Mux;
use failure::Error;
use guiloop::GuiSystem;
use std::rc::Rc;

pub mod guicommon;
pub mod guiloop;

pub mod glium;
#[cfg(all(unix, not(feature = "force-glutin"), not(target_os = "macos")))]
pub mod xwindows;

#[derive(Debug, Deserialize, Clone, Copy)]
pub enum FrontEndSelection {
    Glutin,
    X11,
}

impl Default for FrontEndSelection {
    fn default() -> Self {
        if cfg!(feature = "force-glutin") {
            FrontEndSelection::Glutin
        } else if cfg!(all(unix, not(target_os = "macos"))) {
            FrontEndSelection::X11
        } else {
            FrontEndSelection::Glutin
        }
    }
}

impl FrontEndSelection {
    pub fn try_new(self, mux: &Rc<Mux>) -> Result<Rc<GuiSystem>, Error> {
        let system = match self {
            FrontEndSelection::Glutin => glium::glutinloop::GlutinGuiSystem::try_new(mux),
            #[cfg(all(unix, not(target_os = "macos")))]
            FrontEndSelection::X11 => xwindows::x11loop::X11GuiSystem::try_new(mux),
            #[cfg(not(all(unix, not(target_os = "macos"))))]
            FrontEndSelection::X11 => bail!("X11 not compiled in"),
        };
        system
    }

    // TODO: find or build a proc macro for this
    pub fn variants() -> Vec<&'static str> {
        vec!["Glutin", "X11"]
    }
}

impl std::str::FromStr for FrontEndSelection {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_ref() {
            "glutin" => Ok(FrontEndSelection::Glutin),
            "x11" => Ok(FrontEndSelection::X11),
            _ => Err(format_err!(
                "{} is not a valid FrontEndSelection variant, possible values are {:?}",
                s,
                FrontEndSelection::variants()
            )),
        }
    }
}
