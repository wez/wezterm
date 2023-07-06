use crate::auth::*;
use crate::config::ConfigMap;
use crate::host::*;
use crate::pty::*;
use crate::sessioninner::*;
use crate::sftp::{Sftp, SftpRequest};
use filedescriptor::{socketpair, FileDescriptor};
use portable_pty::PtySize;
use smol::channel::{bounded, Receiver, Sender};
use std::collections::HashMap;
use std::io::Write;
use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub enum SessionEvent {
    Banner(Option<String>),
    HostVerify(HostVerificationEvent),
    Authenticate(AuthenticationEvent),
    HostVerificationFailed(HostVerificationFailed),
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

#[derive(thiserror::Error, Debug)]
#[error("SSH session is dead")]
pub struct DeadSession;

#[derive(Debug)]
pub(crate) enum SessionRequest {
    NewPty(NewPty, Sender<anyhow::Result<(SshPty, SshChildProcess)>>),
    ResizePty(ResizePty, Option<Sender<anyhow::Result<()>>>),
    Exec(Exec, Sender<anyhow::Result<ExecResult>>),
    Sftp(SftpRequest),
    SignalChannel(SignalChannel),
    SessionDropped,
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
}

#[derive(Clone)]
pub struct Session {
    tx: SessionSender,
}

impl Drop for Session {
    fn drop(&mut self) {
        self.tx.try_send(SessionRequest::SessionDropped).ok();
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
            dirs: HashMap::new(),
            next_channel_id: 1,
            next_file_id: 1,
            sender_read,
            session_was_dropped: false,
            shown_accept_env_error: false,
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
            .send(SessionRequest::NewPty(
                NewPty {
                    term: term.to_string(),
                    size,
                    command_line: command_line.map(|s| s.to_string()),
                    env,
                },
                reply,
            ))
            .await
            .map_err(|_| DeadSession)?;
        let (mut ssh_pty, mut child) = rx.recv().await??;
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
            .send(SessionRequest::Exec(
                Exec {
                    command_line: command_line.to_string(),
                    env,
                },
                reply,
            ))
            .await
            .map_err(|_| DeadSession)?;
        let mut exec = rx.recv().await??;
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
