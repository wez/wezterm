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

/// Represents the size of the visible display area in the pty
#[derive(Debug, Clone, Copy)]
pub struct PtySize {
    /// The number of lines of text
    pub rows: u16,
    /// The number of columns of text
    pub cols: u16,
    /// The width of a cell in pixels.  Note that some systems never
    /// fill this value and ignore it.
    pub pixel_width: u16,
    /// The height of a cell in pixels.  Note that some systems never
    /// fill this value and ignore it.
    pub pixel_height: u16,
}

/// Represents the master/control end of the pty
pub trait MasterPty: std::io::Write {
    /// Inform the kernel and thus the child process that the window resized.
    /// It will update the winsize information maintained by the kernel,
    /// and generate a signal for the child to notice and update its state.
    fn resize(&self, size: PtySize) -> Result<(), Error>;
    /// Retrieves the size of the pty as known by the kernel
    fn get_size(&self) -> Result<PtySize, Error>;
    /// Obtain a readable handle; output from the slave(s) is readable
    /// via this stream.
    fn try_clone_reader(&self) -> Result<Box<std::io::Read + Send>, Error>;
}

/// Represents a child process spawned into the pty.
/// This handle can be used to wait for or terminate that child process.
pub trait Child: std::fmt::Debug {
    /// Poll the child to see if it has completed.
    /// Does not block.
    /// Returns None if the has not yet terminated,
    /// else returns its exit status.
    fn try_wait(&mut self) -> IoResult<Option<ExitStatus>>;
    /// Terminate the child process
    fn kill(&mut self) -> IoResult<()>;
    /// Blocks execution until the child process has completed,
    /// yielding its exit status.
    fn wait(&mut self) -> IoResult<ExitStatus>;
}

/// Represents the slave side of a pty.
/// Can be used to spawn processes into the pty.
pub trait SlavePty {
    /// Spawns the command specified by the provided CommandBuilder
    fn spawn_command(&self, cmd: CommandBuilder) -> Result<Box<Child>, Error>;
}

/// Represents the exit status of a child process.
/// This is rather anemic in the current version of this crate,
/// holding only an indicator of success or failure.
#[derive(Debug)]
pub struct ExitStatus {
    successful: bool,
}

impl ExitStatus {
    /// Construct an ExitStatus from a process return code
    pub fn with_exit_code(code: u32) -> Self {
        Self {
            successful: if code == 0 { true } else { false },
        }
    }

    pub fn success(&self) -> bool {
        self.successful
    }
}

impl From<std::process::ExitStatus> for ExitStatus {
    fn from(status: std::process::ExitStatus) -> ExitStatus {
        ExitStatus {
            successful: status.success(),
        }
    }
}

/// The `PtySystem` trait allows an application to work with multiple
/// possible Pty implementations at runtime.  This is important on
/// Windows systems which have a variety of implementations.
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

/// `PtySystemSelection` allows selecting and constructing one of the
/// pty implementations provided by this crate.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde_support", derive(Deserialize))]
pub enum PtySystemSelection {
    /// The Unix style pty interface
    Unix,
    /// The Windows 10+ native Console Pty interface
    ConPty,
    /// rprichard's WinPty interface to cygwin and msys pty.
    /// This requires that winpty.dll be resolvable by the
    /// embedding application.  Instructions on obtaining
    /// an appropriate implementation of winpty.dll can be
    /// found here:
    /// https://github.com/rprichard/winpty
    WinPty,
}

impl PtySystemSelection {
    /// Construct an instance of PtySystem described by the enum value.
    /// Windows specific enum variants result in an error.
    #[cfg(unix)]
    pub fn get(&self) -> Result<Box<PtySystem>, Error> {
        match self {
            PtySystemSelection::Unix => Ok(Box::new(unix::UnixPtySystem {})),
            _ => bail!("{:?} not available on unix", self),
        }
    }

    /// Construct an instance of PtySystem described by the enum value.
    /// Unix specific enum variants result in an error.
    #[cfg(windows)]
    pub fn get(&self) -> Result<Box<PtySystem>, Error> {
        match self {
            PtySystemSelection::ConPty => Ok(Box::new(win::conpty::ConPtySystem {})),
            PtySystemSelection::WinPty => Ok(Box::new(win::winpty::WinPtySystem {})),
            _ => bail!("{:?} not available on Windows", self),
        }
    }

    /// Returns a list of the variant names.
    /// This can be useful for example to specify the list of allowable
    /// options in a clap argument specification.
    pub fn variants() -> Vec<&'static str> {
        vec!["Unix", "ConPty", "WinPty"]
    }
}

/// Parse a string into a `PtySystemSelection` value.
/// This is useful when parsing arguments or configuration files.
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
    /// Returns the default, system native PtySystemSelection
    fn default() -> PtySystemSelection {
        #[cfg(unix)]
        return PtySystemSelection::Unix;
        #[cfg(windows)]
        return PtySystemSelection::ConPty;
    }
}
