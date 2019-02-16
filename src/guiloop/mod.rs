use failure::Error;
use std::process::ExitStatus;

#[cfg(any(windows, target_os = "macos"))]
mod glutinloop;

#[cfg(any(windows, target_os = "macos"))]
pub use glutinloop::{GuiEventLoop, GuiSender};

#[cfg(any(windows, target_os = "macos"))]
pub use gliumwindows::TerminalWindow;

#[cfg(any(windows, target_os = "macos"))]
pub use mpsc::Receiver as GuiReceiver;

#[cfg(any(windows, target_os = "macos"))]
pub use glium::glutin::WindowId;

#[cfg(all(unix, not(target_os = "macos")))]
pub use xwindows::xwin::TerminalWindow;

#[cfg(all(unix, not(target_os = "macos")))]
mod x11;

#[cfg(all(unix, not(target_os = "macos")))]
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
