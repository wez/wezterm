#![cfg(all(unix, not(target_os = "macos")))]

pub mod connection;
pub mod inputhandler;
// pub mod output;
pub mod window;
pub use self::window::*;
pub use connection::*;
// pub use output::*;
mod copy_and_paste;
mod drag_and_drop;
// mod frame;
mod data_device;
mod keyboard;
mod pointer;
mod seat;
mod state;

/// Returns the id of a wayland proxy object, suitable for using
/// a key into hash maps
pub fn todo() {}
// pub fn wl_id<I, T>(obj: T) -> u32
// where
//     I: wayland_client::Interface,
//     T: AsRef<wayland_client::Proxy<I>>,
//     I: AsRef<wayland_client::Proxy<I>>,
//     I: From<wayland_client::Proxy<I>>,
// {
//     let proxy: &wayland_client::Proxy<I> = obj.as_ref();
//     proxy.id()
// }
