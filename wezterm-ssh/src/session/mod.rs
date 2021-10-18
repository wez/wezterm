use crate::auth::*;
use crate::config::ConfigMap;
use crate::host::*;
use crate::pty::*;
pub(crate) use crate::session::inner::*;
use camino::Utf8PathBuf;
use filedescriptor::{
    socketpair, AsRawSocketDescriptor, FileDescriptor, SocketDescriptor, POLLIN, POLLOUT,
};
use libssh_rs as libssh;
use portable_pty::{ExitStatus, PtySize};
use smol::channel::{bounded, Receiver, Sender};
use ssh2::BlockDirections;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::io::Write;
use std::sync::{Arc, Mutex};
use crate::sftp::{Metadata, Sftp, SftpChannelError, SftpChannelResult, SftpRequest};

mod inner;

#[derive(Debug)]
pub enum SessionEvent {
    Banner(Option<String>),
    HostVerify(HostVerificationEvent),
    Authenticate(AuthenticationEvent),
    Error(String),
    Authenticated,
}

#[derive(Debug, Clone)]
pub(crate) struct SessionSender {
    pub tx: Sender<SessionRequest>,
    pub pipe: Arc<Mutex<FileDescriptor>>,
}

impl SessionSender {
    fn post_send(&self) {
        let mut pipe = self.pipe.lock().unwrap();
        let _ = pipe.write(b"x");
    }

    pub fn try_send(&self, event: SessionRequest) -> anyhow::Result<()> {
        self.tx.try_send(event)?;
        self.post_send();
        Ok(())
    }

    pub async fn send(&self, event: SessionRequest) -> anyhow::Result<()> {
        self.tx.send(event).await?;
        self.post_send();
        Ok(())
    }
}

#[derive(Debug)]
pub(crate) enum SessionRequest {
    NewPty(NewPty),
    ResizePty(ResizePty),
    Exec(Exec),
    Sftp(SftpRequest),
    SignalChannel(SignalChannel),
}

#[derive(Debug)]
pub(crate) struct SignalChannel {
    pub channel: ChannelId,
    pub signame: &'static str,
}

#[derive(Debug)]
pub(crate) struct Exec {
    pub command_line: String,
    pub env: Option<HashMap<String, String>>,
    pub reply: Sender<ExecResult>,
}

pub(crate) enum FileWrap {
    Ssh2(ssh2::File),
}

impl FileWrap {
    pub fn reader(&mut self) -> impl std::io::Read + '_ {
        match self {
            Self::Ssh2(file) => file,
        }
    }

    pub fn writer(&mut self) -> impl std::io::Write + '_ {
        match self {
            Self::Ssh2(file) => file,
        }
    }

    pub fn set_metadata(&mut self, metadata: Metadata) -> SftpChannelResult<()> {
        match self {
            Self::Ssh2(file) => file
                .setstat(metadata.into())
                .map_err(SftpChannelError::from),
        }
    }

    pub fn metadata(&mut self) -> SftpChannelResult<Metadata> {
        match self {
            Self::Ssh2(file) => file
                .stat()
                .map(Metadata::from)
                .map_err(SftpChannelError::from),
        }
    }

    pub fn read_dir(&mut self) -> SftpChannelResult<(Utf8PathBuf, Metadata)> {
        match self {
            Self::Ssh2(file) => {
                file.readdir()
                    .map_err(SftpChannelError::from)
                    .and_then(|(path, stat)| match Utf8PathBuf::try_from(path) {
                        Ok(path) => Ok((path, Metadata::from(stat))),
                        Err(x) => Err(SftpChannelError::from(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            x,
                        ))),
                    })
            }
        }
    }

    pub fn fsync(&mut self) -> SftpChannelResult<()> {
        match self {
            Self::Ssh2(file) => file.fsync().map_err(SftpChannelError::from),
        }
    }
}

pub(crate) struct Ssh2Session {
    sess: ssh2::Session,
    sftp: Option<ssh2::Sftp>,
}

pub(crate) enum SessionWrap {
    Ssh2(Ssh2Session),
    LibSsh(libssh::Session),
}

impl SessionWrap {
    pub fn with_ssh2(sess: ssh2::Session) -> Self {
        Self::Ssh2(Ssh2Session { sess, sftp: None })
    }

    pub fn with_libssh(sess: libssh::Session) -> Self {
        Self::LibSsh(sess)
    }

    pub fn set_blocking(&mut self, blocking: bool) {
        match self {
            Self::Ssh2(sess) => sess.sess.set_blocking(blocking),
            Self::LibSsh(sess) => sess.set_blocking(blocking),
        }
    }

    pub fn get_poll_flags(&self) -> i16 {
        match self {
            Self::Ssh2(sess) => match sess.sess.block_directions() {
                BlockDirections::None => 0,
                BlockDirections::Inbound => POLLIN,
                BlockDirections::Outbound => POLLOUT,
                BlockDirections::Both => POLLIN | POLLOUT,
            },
            Self::LibSsh(sess) => {
                let (read, write) = sess.get_poll_state();
                match (read, write) {
                    (false, false) => 0,
                    (true, false) => POLLIN,
                    (false, true) => POLLOUT,
                    (true, true) => POLLIN | POLLOUT,
                }
            }
        }
    }

    pub fn as_socket_descriptor(&self) -> SocketDescriptor {
        match self {
            Self::Ssh2(sess) => sess.sess.as_socket_descriptor(),
            Self::LibSsh(sess) => sess.as_socket_descriptor(),
        }
    }

    pub fn open_session(&self) -> anyhow::Result<ChannelWrap> {
        match self {
            Self::Ssh2(sess) => {
                let channel = sess.sess.channel_session()?;
                Ok(ChannelWrap::Ssh2(channel))
            }
            Self::LibSsh(sess) => {
                let channel = sess.new_channel()?;
                channel.open_session()?;
                Ok(ChannelWrap::LibSsh(channel))
            }
        }
    }
}

pub(crate) enum ChannelWrap {
    Ssh2(ssh2::Channel),
    LibSsh(libssh::Channel),
}

fn has_signal(chan: &ssh2::Channel) -> Option<ssh2::ExitSignal> {
    if let Ok(sig) = chan.exit_signal() {
        if sig.exit_signal.is_some() {
            return Some(sig);
        }
    }
    None
}

impl ChannelWrap {
    pub fn exit_status(&mut self) -> Option<ExitStatus> {
        match self {
            Self::Ssh2(chan) => {
                if chan.eof() && chan.wait_close().is_ok() {
                    if let Some(_sig) = has_signal(chan) {
                        Some(ExitStatus::with_exit_code(1))
                    } else if let Ok(status) = chan.exit_status() {
                        Some(ExitStatus::with_exit_code(status as _))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            Self::LibSsh(chan) => {
                if chan.is_eof() {
                    if let Some(status) = chan.get_exit_status() {
                        return Some(ExitStatus::with_exit_code(status as u32));
                    }
                }
                None
            }
        }
    }

    pub fn reader(&mut self, idx: usize) -> Box<dyn std::io::Read + '_> {
        match self {
            Self::Ssh2(chan) => Box::new(chan.stream(idx as i32)),
            Self::LibSsh(chan) => match idx {
                0 => Box::new(chan.stdout()),
                1 => Box::new(chan.stderr()),
                _ => panic!("wanted reader for idx={}", idx),
            },
        }
    }

    pub fn writer(&mut self) -> Box<dyn std::io::Write + '_> {
        match self {
            Self::Ssh2(chan) => Box::new(chan),
            Self::LibSsh(chan) => Box::new(chan.stdin()),
        }
    }

    pub fn close(&mut self) {
        match self {
            Self::Ssh2(chan) => {
                let _ = chan.close();
            }
            Self::LibSsh(chan) => {
                let _ = chan.close();
            }
        }
    }

    pub fn request_pty(&mut self, newpty: &NewPty) -> anyhow::Result<()> {
        match self {
            Self::Ssh2(chan) => Ok(chan.request_pty(
                &newpty.term,
                None,
                Some((
                    newpty.size.cols.into(),
                    newpty.size.rows.into(),
                    newpty.size.pixel_width.into(),
                    newpty.size.pixel_height.into(),
                )),
            )?),
            Self::LibSsh(chan) => Ok(chan.request_pty(
                &newpty.term,
                newpty.size.cols.into(),
                newpty.size.rows.into(),
            )?),
        }
    }

    pub fn request_env(&mut self, name: &str, value: &str) -> anyhow::Result<()> {
        match self {
            Self::Ssh2(chan) => Ok(chan.setenv(name, value)?),
            Self::LibSsh(chan) => Ok(chan.request_env(name, value)?),
        }
    }

    pub fn request_exec(&mut self, command_line: &str) -> anyhow::Result<()> {
        match self {
            Self::Ssh2(chan) => Ok(chan.exec(command_line)?),
            Self::LibSsh(chan) => Ok(chan.request_exec(command_line)?),
        }
    }

    pub fn request_shell(&mut self) -> anyhow::Result<()> {
        match self {
            Self::Ssh2(chan) => Ok(chan.shell()?),
            Self::LibSsh(chan) => Ok(chan.request_shell()?),
        }
    }

    pub fn resize_pty(&mut self, resize: &ResizePty) -> anyhow::Result<()> {
        match self {
            Self::Ssh2(chan) => Ok(chan.request_pty_size(
                resize.size.cols.into(),
                resize.size.rows.into(),
                Some(resize.size.pixel_width.into()),
                Some(resize.size.pixel_height.into()),
            )?),
            Self::LibSsh(chan) => {
                Ok(chan.change_pty_size(resize.size.cols.into(), resize.size.rows.into())?)
            }
        }
    }

    pub fn send_signal(&mut self, signame: &str) -> anyhow::Result<()> {
        match self {
            Self::Ssh2(_) => Ok(()),
            Self::LibSsh(chan) => Ok(chan.request_send_signal(signame)?),
        }
    }
}

#[derive(Clone)]
pub struct Session {
    tx: SessionSender,
}

impl Drop for Session {
    fn drop(&mut self) {
        log::trace!("Drop Session");
    }
}

impl Session {
    pub fn connect(config: ConfigMap) -> anyhow::Result<(Self, Receiver<SessionEvent>)> {
        let (tx_event, rx_event) = bounded(8);
        let (tx_req, rx_req) = bounded(8);
        let (mut sender_write, mut sender_read) = socketpair()?;
        sender_write.set_non_blocking(true)?;
        sender_read.set_non_blocking(true)?;

        let session_sender = SessionSender {
            tx: tx_req,
            pipe: Arc::new(Mutex::new(sender_write)),
        };

        let mut inner = SessionInner {
            config,
            tx_event,
            rx_req,
            channels: HashMap::new(),
            files: HashMap::new(),
            next_channel_id: 1,
            next_file_id: 1,
            sender_read,
        };
        std::thread::spawn(move || inner.run());
        Ok((Self { tx: session_sender }, rx_event))
    }

    pub async fn request_pty(
        &self,
        term: &str,
        size: PtySize,
        command_line: Option<&str>,
        env: Option<HashMap<String, String>>,
    ) -> anyhow::Result<(SshPty, SshChildProcess)> {
        let (reply, rx) = bounded(1);
        self.tx
            .send(SessionRequest::NewPty(NewPty {
                term: term.to_string(),
                size,
                command_line: command_line.map(|s| s.to_string()),
                env,
                reply,
            }))
            .await?;
        let (mut ssh_pty, mut child) = rx.recv().await?;
        ssh_pty.tx.replace(self.tx.clone());
        child.tx.replace(self.tx.clone());
        Ok((ssh_pty, child))
    }

    pub async fn exec(
        &self,
        command_line: &str,
        env: Option<HashMap<String, String>>,
    ) -> anyhow::Result<ExecResult> {
        let (reply, rx) = bounded(1);
        self.tx
            .send(SessionRequest::Exec(Exec {
                command_line: command_line.to_string(),
                env,
                reply,
            }))
            .await?;
        let mut exec = rx.recv().await?;
        exec.child.tx.replace(self.tx.clone());
        Ok(exec)
    }

    /// Creates a new reference to the sftp channel for filesystem operations
    ///
    /// ### Note
    ///
    /// This does not actually initialize the sftp subsystem and only provides
    /// a reference to a means to perform sftp operations. Upon requesting the
    /// first sftp operation, the sftp subsystem will be initialized.
    pub fn sftp(&self) -> Sftp {
        Sftp {
            tx: self.tx.clone(),
        }
    }
}

#[derive(Debug)]
pub struct ExecResult {
    pub stdin: FileDescriptor,
    pub stdout: FileDescriptor,
    pub stderr: FileDescriptor,
    pub child: SshChildProcess,
}
