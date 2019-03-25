use failure::Error;
use std::io::Result as IoResult;

pub mod cmdbuilder;
pub use cmdbuilder::CommandBuilder;

#[cfg(unix)]
pub mod unix;
#[cfg(windows)]
pub mod win;

#[derive(Debug, Clone, Copy)]
pub struct PtySize {
    pub rows: u16,
    pub cols: u16,
    pub pixel_width: u16,
    pub pixel_height: u16,
}

pub trait MasterPtyTrait: std::io::Write {
    /// Inform the kernel and thus the child process that the window resized.
    /// It will update the winsize information maintained by the kernel,
    /// and generate a signal for the child to notice and update its state.
    fn resize(&self, size: PtySize) -> Result<(), Error>;
    fn get_size(&self) -> Result<PtySize, Error>;
    fn try_clone_reader(&self) -> Result<Box<std::io::Read + Send>, Error>;
}

pub trait ChildTrait: std::fmt::Debug {
    fn try_wait(&mut self) -> IoResult<Option<ExitStatus>>;
    fn kill(&mut self) -> IoResult<()>;
    fn wait(&mut self) -> IoResult<ExitStatus>;
}

pub trait SlavePtyTrait {
    fn spawn_command(&self, cmd: CommandBuilder) -> Result<Box<ChildTrait>, Error>;
}

#[derive(Debug)]
pub struct ExitStatus {
    successful: bool,
}

#[cfg(windows)]
impl ExitStatus {
    pub fn with_exit_code(code: u32) -> Self {
        Self {
            successful: if code == 0 { true } else { false },
        }
    }
}

impl From<std::process::ExitStatus> for ExitStatus {
    fn from(_status: std::process::ExitStatus) -> ExitStatus {
        // FIXME: properly fill
        ExitStatus { successful: true }
    }
}

pub trait PtySystem {
    /// Create a new Pty instance with the window size set to the specified
    /// dimensions.  Returns a (master, slave) Pty pair.  The master side
    /// is used to drive the slave side.
    fn openpty(&self, size: PtySize) -> Result<(Box<MasterPtyTrait>, Box<SlavePtyTrait>), Error>;
}

impl ChildTrait for std::process::Child {
    fn try_wait(&mut self) -> IoResult<Option<ExitStatus>> {
        std::process::Child::try_wait(self).map(|s| match s {
            Some(s) => Some(s.into()),
            None => None,
        })
    }

    fn kill(&mut self) -> IoResult<()> {
        std::process::Child::kill(self)
    }

    fn wait(&mut self) -> IoResult<ExitStatus> {
        std::process::Child::wait(self).map(Into::into)
    }
}

#[cfg(unix)]
pub use self::unix::UnixPtySystem as ThePtySystem;
#[cfg(windows)]
pub use self::win::conpty::ConPtySystem as ThePtySystem;
