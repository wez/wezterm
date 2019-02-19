use super::ExitStatus;
use failure::Error;

#[cfg(any(windows, feature = "force-glutin", target_os = "macos"))]
mod glutinloop;

#[cfg(any(windows, feature = "force-glutin", target_os = "macos"))]
pub use self::glutinloop::{GuiEventLoop, GuiSender, TerminalWindow, WindowId};

#[cfg(any(windows, feature = "force-glutin", target_os = "macos"))]
pub use std::sync::mpsc::Receiver as GuiReceiver;

#[cfg(all(unix, not(feature = "force-glutin"), not(target_os = "macos")))]
pub use crate::xwindows::xwin::TerminalWindow;

#[cfg(all(unix, not(feature = "force-glutin"), not(target_os = "macos")))]
mod x11;

#[cfg(all(unix, not(feature = "force-glutin"), not(target_os = "macos")))]
pub use self::x11::*;

#[derive(Debug, Fail)]
pub enum SessionTerminated {
    #[fail(display = "Process exited: {:?}", status)]
    ProcessStatus { status: ExitStatus },
    #[fail(display = "Error: {:?}", err)]
    Error { err: Error },
    #[fail(display = "Window Closed")]
    WindowClosed,
}
