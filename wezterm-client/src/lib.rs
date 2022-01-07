#[cfg(unix)]
use std::os::unix::net::UnixStream;
#[cfg(windows)]
use uds_windows::UnixStream;

pub mod client;
pub mod discovery;
pub mod domain;
pub mod pane;
