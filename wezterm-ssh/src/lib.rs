#[cfg(not(any(feature = "libssh-rs", feature = "ssh2")))]
compile_error!("Either libssh-rs or ssh2 must be enabled!");

mod auth;
mod channelwrap;
mod config;
mod dirwrap;
mod filewrap;
mod host;
mod pty;
mod session;
mod sessioninner;
mod sessionwrap;
mod sftp;
mod sftpwrap;

pub use auth::*;
pub use config::*;
pub use host::*;
pub use pty::*;
pub use session::*;
pub use sftp::error::*;
pub use sftp::types::*;
pub use sftp::*;

// NOTE: Re-exported as is exposed in a public API of this crate
pub use camino::{Utf8Path, Utf8PathBuf};
pub use filedescriptor::FileDescriptor;
pub use portable_pty::{Child, ChildKiller, MasterPty, PtySize};
