#[cfg(target_os = "macos")]
mod glutinloop;

#[cfg(target_os = "macos")]
pub use glutinloop::{GuiEventLoop, GuiSender};

#[cfg(target_os = "macos")]
pub use mpsc::Receiver as GuiReceiver;

#[cfg(target_os = "macos")]
pub use glium::glutin::WindowId;

#[cfg(all(unix, not(target_os = "macos")))]
pub use mio_extras::channel::{Receiver as GuiReceiver, Sender as GuiSender};

#[cfg(all(unix, not(target_os = "macos")))]
pub use xcb::xproto::Window as WindowId;
