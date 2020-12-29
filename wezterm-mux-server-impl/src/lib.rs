#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};
#[cfg(windows)]
use uds_windows::{UnixListener, UnixStream};

pub mod dispatch;
pub mod local;
pub mod pki;
pub mod sessionhandler;

lazy_static::lazy_static! {
    pub static ref PKI: pki::Pki = pki::Pki::init().expect("failed to initialize PKI");
}
