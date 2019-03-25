#[cfg(windows)]
pub mod conpty;
#[cfg(windows)]
pub mod ownedhandle;
#[cfg(unix)]
pub mod unix;
#[cfg(windows)]
pub mod winpty;

#[cfg(windows)]
pub use self::conpty::{openpty, Child, Command, ExitStatus, MasterPty, SlavePty};
#[cfg(unix)]
pub use self::unix::{openpty, Child, Command, ExitStatus, MasterPty, SlavePty};
