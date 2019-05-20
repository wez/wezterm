use failure::{bail, format_err, Error};
#[cfg(feature = "serde_support")]
use serde_derive::*;
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

pub trait MasterPty: std::io::Write {
    /// Inform the kernel and thus the child process that the window resized.
    /// It will update the winsize information maintained by the kernel,
    /// and generate a signal for the child to notice and update its state.
    fn resize(&self, size: PtySize) -> Result<(), Error>;
    fn get_size(&self) -> Result<PtySize, Error>;
    fn try_clone_reader(&self) -> Result<Box<std::io::Read + Send>, Error>;
}

pub trait Child: std::fmt::Debug {
    fn try_wait(&mut self) -> IoResult<Option<ExitStatus>>;
    fn kill(&mut self) -> IoResult<()>;
    fn wait(&mut self) -> IoResult<ExitStatus>;
}

pub trait SlavePty {
    fn spawn_command(&self, cmd: CommandBuilder) -> Result<Box<Child>, Error>;
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
    fn openpty(&self, size: PtySize) -> Result<(Box<MasterPty>, Box<SlavePty>), Error>;
}

impl Child for std::process::Child {
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

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde_support", derive(Deserialize))]
pub enum PtySystemSelection {
    Unix,
    ConPty,
    WinPty,
}

impl PtySystemSelection {
    #[cfg(unix)]
    pub fn get(&self) -> Result<Box<PtySystem>, Error> {
        match self {
            PtySystemSelection::Unix => Ok(Box::new(unix::UnixPtySystem {})),
            _ => bail!("{:?} not available on unix", self),
        }
    }
    #[cfg(windows)]
    pub fn get(&self) -> Result<Box<PtySystem>, Error> {
        match self {
            PtySystemSelection::ConPty => Ok(Box::new(win::conpty::ConPtySystem {})),
            PtySystemSelection::WinPty => Ok(Box::new(win::winpty::WinPtySystem {})),
            _ => bail!("{:?} not available on Windows", self),
        }
    }

    pub fn variants() -> Vec<&'static str> {
        vec!["Unix", "ConPty", "WinPty"]
    }
}

impl std::str::FromStr for PtySystemSelection {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_ref() {
            "unix" => Ok(PtySystemSelection::Unix),
            "winpty" => Ok(PtySystemSelection::WinPty),
            "conpty" => Ok(PtySystemSelection::ConPty),
            _ => Err(format_err!(
                "{} is not a valid PtySystemSelection variant, possible values are {:?}",
                s,
                PtySystemSelection::variants()
            )),
        }
    }
}

impl Default for PtySystemSelection {
    fn default() -> PtySystemSelection {
        #[cfg(unix)]
        return PtySystemSelection::Unix;
        #[cfg(windows)]
        return PtySystemSelection::ConPty;
    }
}
