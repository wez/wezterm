use crate::auth::*;
use crate::config::ConfigMap;
use crate::host::*;
use crate::pty::*;
use anyhow::{anyhow, Context};
use camino::Utf8PathBuf;
use filedescriptor::{
    poll, pollfd, socketpair, AsRawSocketDescriptor, FileDescriptor, SocketDescriptor, POLLIN,
    POLLOUT,
};
use libssh_rs as libssh;
use portable_pty::{ExitStatus, PtySize};
use smol::channel::{bounded, Receiver, Sender, TryRecvError};
use ssh2::BlockDirections;
use std::collections::{HashMap, VecDeque};
use std::convert::TryFrom;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::time::Duration;

mod sftp;
pub use sftp::{
    File, FilePermissions, FileType, Metadata, OpenFileType, OpenOptions, RenameOptions, Sftp,
    SftpChannelError, SftpChannelResult, SftpError, SftpResult, WriteMode,
};
use sftp::{FileId, FileRequest, SftpRequest};

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

#[derive(Debug)]
pub(crate) struct DescriptorState {
    pub fd: Option<FileDescriptor>,
    pub buf: VecDeque<u8>,
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

pub(crate) struct ChannelInfo {
    pub channel_id: ChannelId,
    pub channel: ChannelWrap,
    pub exit: Option<Sender<ExitStatus>>,
    pub descriptors: [DescriptorState; 3],
}

pub(crate) type ChannelId = usize;

pub(crate) struct SessionInner {
    pub config: ConfigMap,
    pub tx_event: Sender<SessionEvent>,
    pub rx_req: Receiver<SessionRequest>,
    pub channels: HashMap<ChannelId, ChannelInfo>,
    pub files: HashMap<FileId, FileWrap>,
    pub next_channel_id: ChannelId,
    pub next_file_id: FileId,
    pub sender_read: FileDescriptor,
}

impl Drop for SessionInner {
    fn drop(&mut self) {
        log::trace!("Dropping SessionInner");
    }
}

impl SessionInner {
    fn run(&mut self) {
        if let Err(err) = self.run_impl() {
            self.tx_event
                .try_send(SessionEvent::Error(format!("{:#}", err)))
                .ok();
        }
    }

    fn run_impl(&mut self) -> anyhow::Result<()> {
        if true {
            self.run_impl_libssh()
        } else {
            self.run_impl_ssh2()
        }
    }

    fn run_impl_libssh(&mut self) -> anyhow::Result<()> {
        let hostname = self
            .config
            .get("hostname")
            .ok_or_else(|| anyhow!("hostname not present in config"))?
            .to_string();
        let user = self
            .config
            .get("user")
            .ok_or_else(|| anyhow!("username not present in config"))?
            .to_string();
        let port = self
            .config
            .get("port")
            .ok_or_else(|| anyhow!("port is always set in config loader"))?
            .parse::<u16>()?;

        let sess = libssh::Session::new()?;
        // sess.set_option(libssh::SshOption::LogLevel(libssh::LogLevel::Packet))?;
        sess.set_option(libssh::SshOption::Hostname(hostname.clone()))?;
        sess.set_option(libssh::SshOption::User(Some(user)))?;
        sess.set_option(libssh::SshOption::Port(port))?;
        sess.options_parse_config(None)?; // FIXME: overridden config path?
        sess.connect()?;

        let banner = sess.get_server_banner()?;
        self.tx_event
            .try_send(SessionEvent::Banner(Some(banner)))
            .context("notifying user of banner")?;

        self.host_verification_libssh(&sess, &hostname, port)?;
        self.authenticate_libssh(&sess)?;

        if let Ok(banner) = sess.get_issue_banner() {
            self.tx_event
                .try_send(SessionEvent::Banner(Some(banner)))
                .context("notifying user of banner")?;
        }

        self.tx_event
            .try_send(SessionEvent::Authenticated)
            .context("notifying user that session is authenticated")?;

        sess.set_blocking(false);
        let mut sess = SessionWrap::with_libssh(sess);
        self.request_loop(&mut sess)
    }

    fn run_impl_ssh2(&mut self) -> anyhow::Result<()> {
        let hostname = self
            .config
            .get("hostname")
            .ok_or_else(|| anyhow!("hostname not present in config"))?
            .to_string();
        let user = self
            .config
            .get("user")
            .ok_or_else(|| anyhow!("username not present in config"))?
            .to_string();
        let port = self
            .config
            .get("port")
            .ok_or_else(|| anyhow!("port is always set in config loader"))?
            .parse::<u16>()?;
        let remote_address = format!("{}:{}", hostname, port);

        let tcp: TcpStream = if let Some(proxy_command) =
            self.config.get("proxycommand").and_then(|c| {
                if !c.is_empty() && c != "none" {
                    Some(c)
                } else {
                    None
                }
            }) {
            let mut cmd;
            if cfg!(windows) {
                let comspec = std::env::var("COMSPEC").unwrap_or_else(|_| "cmd".to_string());
                cmd = std::process::Command::new(comspec);
                cmd.args(&["/c", proxy_command]);
            } else {
                cmd = std::process::Command::new("sh");
                cmd.args(&["-c", &format!("exec {}", proxy_command)]);
            }

            let (a, b) = socketpair()?;

            cmd.stdin(b.as_stdio()?);
            cmd.stdout(b.as_stdio()?);
            cmd.stderr(std::process::Stdio::inherit());
            let _child = cmd
                .spawn()
                .with_context(|| format!("spawning ProxyCommand {}", proxy_command))?;

            #[cfg(unix)]
            unsafe {
                use std::os::unix::io::{FromRawFd, IntoRawFd};
                TcpStream::from_raw_fd(a.into_raw_fd())
            }
            #[cfg(windows)]
            unsafe {
                use std::os::windows::io::{FromRawSocket, IntoRawSocket};
                TcpStream::from_raw_socket(a.into_raw_socket())
            }
        } else {
            let socket = TcpStream::connect((hostname.as_str(), port))
                .with_context(|| format!("connecting to {}", remote_address))?;
            socket
                .set_nodelay(true)
                .context("setting TCP NODELAY on ssh connection")?;
            socket
        };

        let mut sess = ssh2::Session::new()?;
        // sess.trace(ssh2::TraceFlags::all());
        sess.set_blocking(true);
        sess.set_tcp_stream(tcp);
        sess.handshake()
            .with_context(|| format!("ssh handshake with {}", remote_address))?;

        self.tx_event
            .try_send(SessionEvent::Banner(sess.banner().map(|s| s.to_string())))
            .context("notifying user of banner")?;

        self.host_verification(&sess, &hostname, port, &remote_address)
            .context("host verification")?;

        self.authenticate(&sess, &user, &hostname)
            .context("authentication")?;

        self.tx_event
            .try_send(SessionEvent::Authenticated)
            .context("notifying user that session is authenticated")?;

        sess.set_blocking(false);

        let mut sess = SessionWrap::with_ssh2(sess);
        self.request_loop(&mut sess)
    }

    fn request_loop(&mut self, sess: &mut SessionWrap) -> anyhow::Result<()> {
        let mut sleep_delay = Duration::from_millis(100);

        loop {
            self.tick_io()?;
            self.drain_request_pipe();
            self.dispatch_pending_requests(sess)?;

            let mut poll_array = vec![
                pollfd {
                    fd: self.sender_read.as_socket_descriptor(),
                    events: POLLIN,
                    revents: 0,
                },
                pollfd {
                    fd: sess.as_socket_descriptor(),
                    events: sess.get_poll_flags(),
                    revents: 0,
                },
            ];
            let mut mapping = vec![];

            for info in self.channels.values() {
                for (fd_num, state) in info.descriptors.iter().enumerate() {
                    if let Some(fd) = state.fd.as_ref() {
                        poll_array.push(pollfd {
                            fd: fd.as_socket_descriptor(),
                            events: if fd_num == 0 {
                                POLLIN
                            } else if !state.buf.is_empty() {
                                POLLOUT
                            } else {
                                0
                            },
                            revents: 0,
                        });
                        mapping.push((info.channel_id, fd_num));
                    }
                }
            }

            poll(&mut poll_array, Some(sleep_delay)).context("poll")?;
            sleep_delay += sleep_delay;

            for (idx, poll) in poll_array.iter().enumerate() {
                if poll.revents != 0 {
                    sleep_delay = Duration::from_millis(100);
                }
                if idx == 0 || idx == 1 {
                    // Dealt with at the top of the loop
                } else if poll.revents != 0 {
                    let (channel_id, fd_num) = mapping[idx - 2];
                    let info = self.channels.get_mut(&channel_id).unwrap();
                    let state = &mut info.descriptors[fd_num];
                    let fd = state.fd.as_mut().unwrap();

                    if fd_num == 0 {
                        // There's data we can read into the buffer
                        match read_into_buf(fd, &mut state.buf) {
                            Ok(_) => {}
                            Err(err) => {
                                log::debug!("error reading from stdin pipe: {:#}", err);
                                info.channel.close();
                                state.fd.take();
                            }
                        }
                    } else {
                        // We can write our buffered output
                        match write_from_buf(fd, &mut state.buf) {
                            Ok(_) => {}
                            Err(err) => {
                                log::debug!(
                                    "error while writing to channel {} fd {}: {:#}",
                                    channel_id,
                                    fd_num,
                                    err
                                );

                                // Close it out
                                state.fd.take();
                            }
                        }
                    }
                }
            }
        }
    }

    /// Goal: if we have data to write to channels, try to send it.
    /// If we have room in our channel fd write buffers, try to fill it
    fn tick_io(&mut self) -> anyhow::Result<()> {
        for chan in self.channels.values_mut() {
            if chan.exit.is_some() {
                if let Some(status) = chan.channel.exit_status() {
                    let exit = chan.exit.take().unwrap();
                    smol::block_on(exit.send(status)).ok();
                }
            }

            let stdin = &mut chan.descriptors[0];
            if stdin.fd.is_some() && !stdin.buf.is_empty() {
                write_from_buf(&mut chan.channel.writer(), &mut stdin.buf)
                    .context("writing to channel")?;
            }

            for (idx, out) in chan
                .descriptors
                .get_mut(1..)
                .unwrap()
                .iter_mut()
                .enumerate()
            {
                if out.fd.is_none() {
                    continue;
                }
                let current_len = out.buf.len();
                let room = out.buf.capacity() - current_len;
                if room == 0 {
                    continue;
                }
                match read_into_buf(&mut chan.channel.reader(idx), &mut out.buf) {
                    Ok(_) => {}
                    Err(err) => {
                        if out.buf.is_empty() {
                            log::trace!(
                                "Failed to read data from channel: {:#}, closing pipe",
                                err
                            );
                            out.fd.take();
                        } else {
                            log::trace!("Failed to read data from channel: {:#}, but still have some buffer to drain", err);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn drain_request_pipe(&mut self) {
        let mut buf = [0u8; 16];
        let _ = self.sender_read.read(&mut buf);
    }

    fn dispatch_pending_requests(&mut self, sess: &mut SessionWrap) -> anyhow::Result<()> {
        while self.dispatch_one_request(sess)? {}
        Ok(())
    }

    fn dispatch_one_request(&mut self, sess: &mut SessionWrap) -> anyhow::Result<bool> {
        match self.rx_req.try_recv() {
            Err(TryRecvError::Closed) => anyhow::bail!("all clients are closed"),
            Err(TryRecvError::Empty) => Ok(false),
            Ok(req) => {
                sess.set_blocking(true);
                let res = match req {
                    SessionRequest::NewPty(newpty) => {
                        if let Err(err) = self.new_pty(sess, &newpty) {
                            log::error!("{:?} -> error: {:#}", newpty, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::ResizePty(resize) => {
                        if let Err(err) = self.resize_pty(&resize) {
                            log::error!("{:?} -> error: {:#}", resize, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Exec(exec) => {
                        if let Err(err) = self.exec(sess, &exec) {
                            log::error!("{:?} -> error: {:#}", exec, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::SignalChannel(info) => {
                        if let Err(err) = self.signal_channel(&info) {
                            log::error!("{:?} -> error: {:#}", info, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::OpenWithMode(msg)) => {
                        if let Err(err) = self.open_with_mode(sess, &msg) {
                            log::error!("{:?} -> error: {:#}", msg, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::Open(msg)) => {
                        if let Err(err) = self.open(sess, &msg) {
                            log::error!("{:?} -> error: {:#}", msg, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::Create(msg)) => {
                        if let Err(err) = self.create(sess, &msg) {
                            log::error!("{:?} -> error: {:#}", msg, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::OpenDir(msg)) => {
                        if let Err(err) = self.open_dir(sess, &msg) {
                            log::error!("{:?} -> error: {:#}", msg, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::File(FileRequest::Write(msg))) => {
                        if let Err(err) = self.write_file(sess, &msg) {
                            log::error!("{:?} -> error: {:#}", msg, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::File(FileRequest::Read(msg))) => {
                        if let Err(err) = self.read_file(sess, &msg) {
                            log::error!("{:?} -> error: {:#}", msg, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::File(FileRequest::Close(msg))) => {
                        if let Err(err) = self.close_file(sess, &msg) {
                            log::error!("{:?} -> error: {:#}", msg, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::File(FileRequest::Flush(msg))) => {
                        if let Err(err) = self.flush_file(sess, &msg) {
                            log::error!("{:?} -> error: {:#}", msg, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::File(FileRequest::SetMetadata(msg))) => {
                        if let Err(err) = self.set_metadata_file(sess, &msg) {
                            log::error!("{:?} -> error: {:#}", msg, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::File(FileRequest::Metadata(msg))) => {
                        if let Err(err) = self.metadata_file(sess, &msg) {
                            log::error!("{:?} -> error: {:#}", msg, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::File(FileRequest::ReadDir(msg))) => {
                        if let Err(err) = self.read_dir_file(sess, &msg) {
                            log::error!("{:?} -> error: {:#}", msg, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::File(FileRequest::Fsync(msg))) => {
                        if let Err(err) = self.fsync_file(sess, &msg) {
                            log::error!("{:?} -> error: {:#}", msg, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::ReadDir(msg)) => {
                        if let Err(err) = self.read_dir(sess, &msg) {
                            log::error!("{:?} -> error: {:#}", msg, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::CreateDir(msg)) => {
                        if let Err(err) = self.create_dir(sess, &msg) {
                            log::error!("{:?} -> error: {:#}", msg, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::RemoveDir(msg)) => {
                        if let Err(err) = self.remove_dir(sess, &msg) {
                            log::error!("{:?} -> error: {:#}", msg, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::Metadata(msg)) => {
                        if let Err(err) = self.metadata(sess, &msg) {
                            log::error!("{:?} -> error: {:#}", msg, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::SymlinkMetadata(msg)) => {
                        if let Err(err) = self.symlink_metadata(sess, &msg) {
                            log::error!("{:?} -> error: {:#}", msg, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::SetMetadata(msg)) => {
                        if let Err(err) = self.set_metadata(sess, &msg) {
                            log::error!("{:?} -> error: {:#}", msg, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::Symlink(msg)) => {
                        if let Err(err) = self.symlink(sess, &msg) {
                            log::error!("{:?} -> error: {:#}", msg, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::ReadLink(msg)) => {
                        if let Err(err) = self.read_link(sess, &msg) {
                            log::error!("{:?} -> error: {:#}", msg, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::Canonicalize(msg)) => {
                        if let Err(err) = self.canonicalize(sess, &msg) {
                            log::error!("{:?} -> error: {:#}", msg, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::Rename(msg)) => {
                        if let Err(err) = self.rename(sess, &msg) {
                            log::error!("{:?} -> error: {:#}", msg, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::RemoveFile(msg)) => {
                        if let Err(err) = self.remove_file(sess, &msg) {
                            log::error!("{:?} -> error: {:#}", msg, err);
                        }
                        Ok(true)
                    }
                };
                sess.set_blocking(false);
                res
            }
        }
    }

    pub fn signal_channel(&mut self, info: &SignalChannel) -> anyhow::Result<()> {
        let chan_info = self
            .channels
            .get_mut(&info.channel)
            .ok_or_else(|| anyhow::anyhow!("invalid channel id {}", info.channel))?;
        chan_info.channel.send_signal(info.signame)?;
        Ok(())
    }

    pub fn exec(&mut self, sess: &mut SessionWrap, exec: &Exec) -> anyhow::Result<()> {
        let mut channel = sess.open_session()?;

        if let Some(env) = &exec.env {
            for (key, val) in env {
                if let Err(err) = channel.request_env(key, val) {
                    // Depending on the server configuration, a given
                    // setenv request may not succeed, but that doesn't
                    // prevent the connection from being set up.
                    log::warn!(
                        "ssh: setenv {}={} failed: {}. \
                         Check the AcceptEnv setting on the ssh server side.",
                        key,
                        val,
                        err
                    );
                }
            }
        }

        channel.request_exec(&exec.command_line)?;

        let channel_id = self.next_channel_id;
        self.next_channel_id += 1;

        let (write_to_stdin, mut read_from_stdin) = socketpair()?;
        let (mut write_to_stdout, read_from_stdout) = socketpair()?;
        let (mut write_to_stderr, read_from_stderr) = socketpair()?;

        read_from_stdin.set_non_blocking(true)?;
        write_to_stdout.set_non_blocking(true)?;
        write_to_stderr.set_non_blocking(true)?;

        let (exit_tx, exit_rx) = bounded(1);

        let child = SshChildProcess {
            channel: channel_id,
            tx: None,
            exit: exit_rx,
            exited: None,
        };

        let result = ExecResult {
            stdin: write_to_stdin,
            stdout: read_from_stdout,
            stderr: read_from_stderr,
            child,
        };

        let info = ChannelInfo {
            channel_id,
            channel,
            exit: Some(exit_tx),
            descriptors: [
                DescriptorState {
                    fd: Some(read_from_stdin),
                    buf: VecDeque::with_capacity(8192),
                },
                DescriptorState {
                    fd: Some(write_to_stdout),
                    buf: VecDeque::with_capacity(8192),
                },
                DescriptorState {
                    fd: Some(write_to_stderr),
                    buf: VecDeque::with_capacity(8192),
                },
            ],
        };

        exec.reply.try_send(result)?;
        self.channels.insert(channel_id, info);

        Ok(())
    }

    /// Open a handle to a file.
    pub fn open_with_mode(
        &mut self,
        sess: &mut SessionWrap,
        msg: &sftp::OpenWithMode,
    ) -> anyhow::Result<()> {
        let flags: ssh2::OpenFlags = msg.opts.into();
        let mode = msg.opts.mode;
        let open_type: ssh2::OpenType = msg.opts.ty.into();

        let result = self.init_sftp(sess).and_then(|sftp| {
            sftp.open_mode(msg.filename.as_std_path(), flags, mode, open_type)
                .map_err(SftpChannelError::from)
        });

        match result {
            Ok(ssh_file) => {
                let (file_id, file) = self.make_file();
                msg.reply.try_send(Ok(file))?;
                self.files.insert(file_id, FileWrap::Ssh2(ssh_file));
            }
            Err(x) => msg.reply.try_send(Err(x))?,
        }

        Ok(())
    }

    /// Helper to open a file in the `Read` mode.
    pub fn open(&mut self, sess: &mut SessionWrap, msg: &sftp::Open) -> anyhow::Result<()> {
        let result = self.init_sftp(sess).and_then(|sftp| {
            sftp.open(msg.filename.as_std_path())
                .map_err(SftpChannelError::from)
        });

        match result {
            Ok(ssh_file) => {
                let (file_id, file) = self.make_file();
                msg.reply.try_send(Ok(file))?;
                self.files.insert(file_id, FileWrap::Ssh2(ssh_file));
            }
            Err(x) => msg.reply.try_send(Err(x))?,
        }

        Ok(())
    }

    /// Helper to create a file in write-only mode with truncation.
    pub fn create(&mut self, sess: &mut SessionWrap, msg: &sftp::Create) -> anyhow::Result<()> {
        let result = self.init_sftp(sess).and_then(|sftp| {
            sftp.create(msg.filename.as_std_path())
                .map_err(SftpChannelError::from)
        });

        match result {
            Ok(ssh_file) => {
                let (file_id, file) = self.make_file();
                msg.reply.try_send(Ok(file))?;
                self.files.insert(file_id, FileWrap::Ssh2(ssh_file));
            }
            Err(x) => msg.reply.try_send(Err(x))?,
        }

        Ok(())
    }

    /// Helper to open a directory for reading its contents.
    pub fn open_dir(&mut self, sess: &mut SessionWrap, msg: &sftp::OpenDir) -> anyhow::Result<()> {
        let result = self.init_sftp(sess).and_then(|sftp| {
            sftp.opendir(msg.filename.as_std_path())
                .map_err(SftpChannelError::from)
        });

        match result {
            Ok(ssh_file) => {
                let (file_id, file) = self.make_file();
                msg.reply.try_send(Ok(file))?;
                self.files.insert(file_id, FileWrap::Ssh2(ssh_file));
            }
            Err(x) => msg.reply.try_send(Err(x))?,
        }

        Ok(())
    }

    fn make_file(&mut self) -> (FileId, File) {
        let file_id = self.next_file_id;
        self.next_file_id += 1;

        let file = File::new(file_id);
        (file_id, file)
    }

    /// Writes to a loaded file.
    fn write_file(&mut self, _sess: &mut SessionWrap, msg: &sftp::WriteFile) -> anyhow::Result<()> {
        let sftp::WriteFile {
            file_id,
            data,
            reply,
        } = msg;

        if let Some(file) = self.files.get_mut(file_id) {
            let result = file
                .writer()
                .write_all(data)
                .map_err(SftpChannelError::from);
            reply.try_send(result)?;
        }

        Ok(())
    }

    /// Reads from a loaded file.
    fn read_file(&mut self, _sess: &mut SessionWrap, msg: &sftp::ReadFile) -> anyhow::Result<()> {
        let sftp::ReadFile {
            file_id,
            max_bytes,
            reply,
        } = msg;

        if let Some(file) = self.files.get_mut(file_id) {
            // TODO: Move this somewhere to avoid re-allocating buffer
            let mut buf = vec![0u8; *max_bytes];
            match file.reader().read(&mut buf).map_err(SftpChannelError::from) {
                Ok(n) => {
                    buf.truncate(n);
                    reply.try_send(Ok(buf))?;
                }
                Err(x) => reply.try_send(Err(x))?,
            }
        }

        Ok(())
    }

    /// Closes a file and removes it from the internal memory.
    fn close_file(&mut self, _sess: &mut SessionWrap, msg: &sftp::CloseFile) -> anyhow::Result<()> {
        self.files.remove(&msg.file_id);
        msg.reply.try_send(Ok(()))?;

        Ok(())
    }

    /// Flushes a file.
    fn flush_file(&mut self, _sess: &mut SessionWrap, msg: &sftp::FlushFile) -> anyhow::Result<()> {
        if let Some(file) = self.files.get_mut(&msg.file_id) {
            let result = file.writer().flush().map_err(SftpChannelError::from);
            msg.reply.try_send(result)?;
        }

        Ok(())
    }

    /// Sets file metadata.
    fn set_metadata_file(
        &mut self,
        _sess: &mut SessionWrap,
        msg: &sftp::SetMetadataFile,
    ) -> anyhow::Result<()> {
        let sftp::SetMetadataFile {
            file_id,
            metadata,
            reply,
        } = msg;

        if let Some(file) = self.files.get_mut(file_id) {
            let result = file.set_metadata(*metadata).map_err(SftpChannelError::from);
            reply.try_send(result)?;
        }

        Ok(())
    }

    /// Gets file stat.
    fn metadata_file(
        &mut self,
        _sess: &mut SessionWrap,
        msg: &sftp::MetadataFile,
    ) -> anyhow::Result<()> {
        if let Some(file) = self.files.get_mut(&msg.file_id) {
            let result = file.metadata();
            msg.reply.try_send(result)?;
        }

        Ok(())
    }

    /// Performs readdir for file.
    fn read_dir_file(
        &mut self,
        _sess: &mut SessionWrap,
        msg: &sftp::ReadDirFile,
    ) -> anyhow::Result<()> {
        if let Some(file) = self.files.get_mut(&msg.file_id) {
            let result = file.read_dir();
            msg.reply.try_send(result)?;
        }

        Ok(())
    }

    /// Fsync file.
    fn fsync_file(
        &mut self,
        _sess: &mut SessionWrap,
        fsync_file: &sftp::FsyncFile,
    ) -> anyhow::Result<()> {
        if let Some(file) = self.files.get_mut(&fsync_file.file_id) {
            let result = file.fsync();
            fsync_file.reply.try_send(result)?;
        }

        Ok(())
    }

    /// Convenience function to read the files in a directory.
    pub fn read_dir(&mut self, sess: &mut SessionWrap, msg: &sftp::ReadDir) -> anyhow::Result<()> {
        let result = self.init_sftp(sess).and_then(|sftp| {
            sftp.readdir(msg.filename.as_std_path())
                .map_err(SftpChannelError::from)
                .and_then(|entries| {
                    let mut mapped_entries = Vec::new();
                    for (path, stat) in entries {
                        match Utf8PathBuf::try_from(path) {
                            Ok(path) => mapped_entries.push((path, Metadata::from(stat))),
                            Err(x) => {
                                return Err(SftpChannelError::from(std::io::Error::new(
                                    std::io::ErrorKind::InvalidData,
                                    x,
                                )));
                            }
                        }
                    }

                    Ok(mapped_entries)
                })
        });
        msg.reply.try_send(result)?;

        Ok(())
    }

    /// Create a directory on the remote filesystem.
    pub fn create_dir(
        &mut self,
        sess: &mut SessionWrap,
        msg: &sftp::CreateDir,
    ) -> anyhow::Result<()> {
        let result = self.init_sftp(sess).and_then(|sftp| {
            sftp.mkdir(msg.filename.as_std_path(), msg.mode)
                .map_err(SftpChannelError::from)
        });
        msg.reply.try_send(result)?;

        Ok(())
    }

    /// Remove a directory from the remote filesystem.
    pub fn remove_dir(
        &mut self,
        sess: &mut SessionWrap,
        msg: &sftp::RemoveDir,
    ) -> anyhow::Result<()> {
        let result = self.init_sftp(sess).and_then(|sftp| {
            sftp.rmdir(msg.filename.as_std_path())
                .map_err(SftpChannelError::from)
        });
        msg.reply.try_send(result)?;

        Ok(())
    }

    /// Get the metadata for a file, performed by stat(2).
    pub fn metadata(
        &mut self,
        sess: &mut SessionWrap,
        msg: &sftp::GetMetadata,
    ) -> anyhow::Result<()> {
        let result = self.init_sftp(sess).and_then(|sftp| {
            sftp.stat(msg.filename.as_std_path())
                .map(Metadata::from)
                .map_err(SftpChannelError::from)
        });
        msg.reply.try_send(result)?;

        Ok(())
    }

    /// Get the metadata for a file, performed by lstat(2).
    pub fn symlink_metadata(
        &mut self,
        sess: &mut SessionWrap,
        msg: &sftp::SymlinkMetadata,
    ) -> anyhow::Result<()> {
        let result = self.init_sftp(sess).and_then(|sftp| {
            sftp.lstat(msg.filename.as_std_path())
                .map(Metadata::from)
                .map_err(SftpChannelError::from)
        });
        msg.reply.try_send(result)?;

        Ok(())
    }

    /// Set the metadata for a file.
    pub fn set_metadata(
        &mut self,
        sess: &mut SessionWrap,
        msg: &sftp::SetMetadata,
    ) -> anyhow::Result<()> {
        let result = self.init_sftp(sess).and_then(|sftp| {
            sftp.setstat(msg.filename.as_std_path(), msg.metadata.into())
                .map_err(SftpChannelError::from)
        });
        msg.reply.try_send(result)?;

        Ok(())
    }

    /// Create symlink at `target` pointing at `path`.
    pub fn symlink(&mut self, sess: &mut SessionWrap, msg: &sftp::Symlink) -> anyhow::Result<()> {
        let result = self.init_sftp(sess).and_then(|sftp| {
            sftp.symlink(msg.path.as_std_path(), msg.target.as_std_path())
                .map_err(SftpChannelError::from)
        });
        msg.reply.try_send(result)?;

        Ok(())
    }

    /// Read a symlink at `path`.
    pub fn read_link(
        &mut self,
        sess: &mut SessionWrap,
        msg: &sftp::ReadLink,
    ) -> anyhow::Result<()> {
        let result = self.init_sftp(sess).and_then(|sftp| {
            sftp.readlink(msg.path.as_std_path())
                .map_err(SftpChannelError::from)
                .and_then(|path| {
                    Utf8PathBuf::try_from(path).map_err(|x| {
                        SftpChannelError::from(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            x,
                        ))
                    })
                })
        });
        msg.reply.try_send(result)?;

        Ok(())
    }

    /// Resolve the real path for `path`.
    pub fn canonicalize(
        &mut self,
        sess: &mut SessionWrap,
        msg: &sftp::Canonicalize,
    ) -> anyhow::Result<()> {
        let result = self.init_sftp(sess).and_then(|sftp| {
            sftp.realpath(msg.path.as_std_path())
                .map_err(SftpChannelError::from)
                .and_then(|path| {
                    Utf8PathBuf::try_from(path).map_err(|x| {
                        SftpChannelError::from(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            x,
                        ))
                    })
                })
        });
        msg.reply.try_send(result)?;

        Ok(())
    }

    /// Rename the filesystem object on the remote filesystem.
    pub fn rename(&mut self, sess: &mut SessionWrap, msg: &sftp::Rename) -> anyhow::Result<()> {
        let result = self.init_sftp(sess).and_then(|sftp| {
            sftp.rename(
                msg.src.as_std_path(),
                msg.dst.as_std_path(),
                Some(msg.opts.into()),
            )
            .map_err(SftpChannelError::from)
        });
        msg.reply.try_send(result)?;

        Ok(())
    }

    /// Remove a file on the remote filesystem.
    pub fn remove_file(
        &mut self,
        sess: &mut SessionWrap,
        msg: &sftp::RemoveFile,
    ) -> anyhow::Result<()> {
        let result = self.init_sftp(sess).and_then(|sftp| {
            sftp.unlink(msg.file.as_std_path())
                .map_err(SftpChannelError::from)
        });
        msg.reply.try_send(result)?;

        Ok(())
    }

    /// Initialize the sftp channel if not already created, returning a mutable reference to it
    fn init_sftp<'a>(
        &mut self,
        sess: &'a mut SessionWrap,
    ) -> SftpChannelResult<&'a mut ssh2::Sftp> {
        match sess {
            SessionWrap::Ssh2(sess) => {
                if sess.sftp.is_none() {
                    sess.sftp = Some(sess.sess.sftp()?);
                }
                Ok(sess.sftp.as_mut().expect("sftp should have been set above"))
            }
            SessionWrap::LibSsh(_) => Err(SftpChannelError::NotImplemented),
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

fn write_from_buf<W: Write>(w: &mut W, buf: &mut VecDeque<u8>) -> std::io::Result<()> {
    match w.write(buf.make_contiguous()) {
        Ok(len) => {
            buf.drain(0..len);
            Ok(())
        }
        Err(err) => {
            if err.kind() == std::io::ErrorKind::WouldBlock {
                return Ok(());
            }
            Err(err)
        }
    }
}

fn read_into_buf<R: Read>(r: &mut R, buf: &mut VecDeque<u8>) -> std::io::Result<()> {
    let current_len = buf.len();
    buf.resize(buf.capacity(), 0);
    let target_buf = &mut buf.make_contiguous()[current_len..];
    match r.read(target_buf) {
        Ok(len) => {
            buf.resize(current_len + len, 0);
            if len == 0 {
                Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "EOF",
                ))
            } else {
                Ok(())
            }
        }
        Err(err) => {
            buf.resize(current_len, 0);

            if err.kind() == std::io::ErrorKind::WouldBlock {
                return Ok(());
            }
            Err(err)
        }
    }
}
