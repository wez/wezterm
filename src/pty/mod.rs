#[cfg(unix)]
pub mod unix;
#[cfg(windows)]
pub mod win;

#[cfg(unix)]
pub use self::unix::{openpty, Child, Command, ExitStatus, MasterPty, SlavePty};

#[cfg(all(windows, not(feature = "use-winpty")))]
pub use self::win::conpty::{openpty, Command, MasterPty, SlavePty};
#[cfg(all(windows, feature = "use-winpty"))]
pub use self::win::winpty::{openpty, Command, MasterPty, SlavePty};
#[cfg(windows)]
pub use self::win::{Child, ExitStatus};
