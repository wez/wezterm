use super::*;
use async_trait::async_trait;
use std::pin::Pin;

/// Represents the master/control end of the pty
#[async_trait(?Send)]
pub trait MasterPty: tokio::io::AsyncWrite {
    /// Inform the kernel and thus the child process that the window resized.
    /// It will update the winsize information maintained by the kernel,
    /// and generate a signal for the child to notice and update its state.
    async fn resize(&self, size: PtySize) -> anyhow::Result<()>;
    /// Retrieves the size of the pty as known by the kernel
    async fn get_size(&self) -> anyhow::Result<PtySize>;
    /// Obtain a readable handle; output from the slave(s) is readable
    /// via this stream.
    fn try_clone_reader(&self) -> anyhow::Result<Pin<Box<dyn tokio::io::AsyncRead + Send>>>;
}

/// Represents a child process spawned into the pty.
/// This handle can be used to wait for or terminate that child process.
/// awaiting the Child yields the ExitStatus when the child completes.
pub trait Child:
    std::fmt::Debug + std::future::Future<Output = anyhow::Result<ExitStatus>>
{
    /// Request termination of the child process
    fn kill(&mut self) -> IoResult<()>;
}

/// Represents the slave side of a pty.
/// Can be used to spawn processes into the pty.
#[async_trait(?Send)]
pub trait SlavePty {
    /// Spawns the command specified by the provided CommandBuilder
    async fn spawn_command(&self, cmd: CommandBuilder) -> anyhow::Result<Pin<Box<dyn Child>>>;
}

pub struct PtyPair {
    // slave is listed first so that it is dropped first.
    // The drop order is stable and specified by rust rfc 1857
    pub slave: Pin<Box<dyn SlavePty>>,
    pub master: Pin<Box<dyn MasterPty>>,
}

/// The `PtySystem` trait allows an application to work with multiple
/// possible Pty implementations at runtime.
#[async_trait(?Send)]
pub trait PtySystem {
    /// Create a new Pty instance with the window size set to the specified
    /// dimensions.  Returns a (master, slave) Pty pair.  The master side
    /// is used to drive the slave side.
    async fn openpty(&self, size: PtySize) -> anyhow::Result<PtyPair>;
}

pub fn native_pty_system() -> Box<dyn PtySystem> {
    Box::new(NativePtySystem::default())
}
