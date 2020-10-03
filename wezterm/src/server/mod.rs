#[cfg(unix)]
use std::os::unix::net::UnixStream;
#[cfg(windows)]
use uds_windows::UnixStream;

pub mod client;
pub mod domain;
pub mod pollable;
pub mod tab;
