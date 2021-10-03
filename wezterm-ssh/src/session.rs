use crate::auth::*;
use crate::config::ConfigMap;
use crate::host::*;
use crate::pty::*;
use anyhow::{anyhow, Context};
use filedescriptor::{
    poll, pollfd, socketpair, AsRawSocketDescriptor, FileDescriptor, POLLIN, POLLOUT,
};
use portable_pty::{ExitStatus, PtySize};
use smol::channel::{bounded, Receiver, Sender, TryRecvError};
use ssh2::BlockDirections;
use std::collections::{HashMap, VecDeque};
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
}

#[derive(Debug)]
pub(crate) struct Exec {
    pub command_line: String,
    pub env: Option<HashMap<String, String>>,
    pub reply: Sender<ExecResult>,
}

pub(crate) struct DescriptorState {
    pub fd: Option<FileDescriptor>,
    pub buf: VecDeque<u8>,
}

pub(crate) struct ChannelInfo {
    pub channel_id: ChannelId,
    pub channel: ssh2::Channel,
    pub exit: Option<Sender<ExitStatus>>,
    pub descriptors: [DescriptorState; 3],
}

pub(crate) type ChannelId = usize;

pub(crate) struct SessionInner {
    pub config: ConfigMap,
    pub tx_event: Sender<SessionEvent>,
    pub rx_req: Receiver<SessionRequest>,
    pub channels: HashMap<ChannelId, ChannelInfo>,
    pub files: HashMap<FileId, ssh2::File>,
    pub sftp: Option<ssh2::Sftp>,
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
        let port = self.config.get("port").unwrap().parse::<u16>()?;
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
        self.request_loop(sess)
    }

    fn request_loop(&mut self, sess: ssh2::Session) -> anyhow::Result<()> {
        let mut sleep_delay = Duration::from_millis(100);

        loop {
            self.tick_io()?;
            self.drain_request_pipe();
            self.dispatch_pending_requests(&sess)?;

            let mut poll_array = vec![
                pollfd {
                    fd: self.sender_read.as_socket_descriptor(),
                    events: POLLIN,
                    revents: 0,
                },
                pollfd {
                    fd: sess.as_socket_descriptor(),
                    events: match sess.block_directions() {
                        BlockDirections::None => 0,
                        BlockDirections::Inbound => POLLIN,
                        BlockDirections::Outbound => POLLOUT,
                        BlockDirections::Both => POLLIN | POLLOUT,
                    },
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
                                let _ = info.channel.close();
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
                if chan.channel.eof() && chan.channel.wait_close().is_ok() {
                    fn has_signal(chan: &ssh2::Channel) -> Option<ssh2::ExitSignal> {
                        if let Ok(sig) = chan.exit_signal() {
                            if sig.exit_signal.is_some() {
                                return Some(sig);
                            }
                        }
                        None
                    }

                    let status = if let Some(_sig) = has_signal(&chan.channel) {
                        Some(ExitStatus::with_exit_code(1))
                    } else if let Ok(status) = chan.channel.exit_status() {
                        Some(ExitStatus::with_exit_code(status as _))
                    } else {
                        None
                    };

                    if let Some(status) = status {
                        let exit = chan.exit.take().unwrap();
                        smol::block_on(exit.send(status)).ok();
                    }
                }
            }

            let stdin = &mut chan.descriptors[0];
            if stdin.fd.is_some() && !stdin.buf.is_empty() {
                write_from_buf(&mut chan.channel, &mut stdin.buf).context("writing to channel")?;
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
                match read_into_buf(&mut chan.channel.stream(idx as i32), &mut out.buf) {
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

    fn dispatch_pending_requests(&mut self, sess: &ssh2::Session) -> anyhow::Result<()> {
        while self.dispatch_one_request(sess)? {}
        Ok(())
    }

    fn dispatch_one_request(&mut self, sess: &ssh2::Session) -> anyhow::Result<bool> {
        match self.rx_req.try_recv() {
            Err(TryRecvError::Closed) => anyhow::bail!("all clients are closed"),
            Err(TryRecvError::Empty) => Ok(false),
            Ok(req) => {
                sess.set_blocking(true);
                let res = match req {
                    SessionRequest::NewPty(newpty) => {
                        if let Err(err) = self.new_pty(&sess, &newpty) {
                            log::error!("{:?} -> error: {:#}", newpty, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::ResizePty(resize) => {
                        if let Err(err) = self.resize_pty(&sess, &resize) {
                            log::error!("{:?} -> error: {:#}", resize, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Exec(exec) => {
                        if let Err(err) = self.exec(&sess, &exec) {
                            log::error!("{:?} -> error: {:#}", exec, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::OpenWithMode(msg)) => {
                        if let Err(err) = self.open_with_mode(&sess, &msg) {
                            log::error!("{:?} -> error: {:#}", msg, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::Open(msg)) => {
                        if let Err(err) = self.open(&sess, &msg) {
                            log::error!("{:?} -> error: {:#}", msg, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::Create(msg)) => {
                        if let Err(err) = self.create(&sess, &msg) {
                            log::error!("{:?} -> error: {:#}", msg, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::OpenDir(msg)) => {
                        if let Err(err) = self.open_dir(&sess, &msg) {
                            log::error!("{:?} -> error: {:#}", msg, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::File(FileRequest::Write(msg))) => {
                        if let Err(err) = self.write_file(&sess, &msg) {
                            log::error!("{:?} -> error: {:#}", msg, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::File(FileRequest::Read(msg))) => {
                        if let Err(err) = self.read_file(&sess, &msg) {
                            log::error!("{:?} -> error: {:#}", msg, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::File(FileRequest::Close(msg))) => {
                        if let Err(err) = self.close_file(&sess, &msg) {
                            log::error!("{:?} -> error: {:#}", msg, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::File(FileRequest::Flush(msg))) => {
                        if let Err(err) = self.flush_file(&sess, &msg) {
                            log::error!("{:?} -> error: {:#}", msg, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::File(FileRequest::SetMetadata(msg))) => {
                        if let Err(err) = self.set_metadata_file(&sess, &msg) {
                            log::error!("{:?} -> error: {:#}", msg, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::File(FileRequest::Metadata(msg))) => {
                        if let Err(err) = self.metadata_file(&sess, &msg) {
                            log::error!("{:?} -> error: {:#}", msg, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::File(FileRequest::ReadDir(msg))) => {
                        if let Err(err) = self.read_dir_file(&sess, &msg) {
                            log::error!("{:?} -> error: {:#}", msg, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::File(FileRequest::Fsync(msg))) => {
                        if let Err(err) = self.fsync_file(&sess, &msg) {
                            log::error!("{:?} -> error: {:#}", msg, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::ReadDir(msg)) => {
                        if let Err(err) = self.read_dir(&sess, &msg) {
                            log::error!("{:?} -> error: {:#}", msg, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::CreateDir(msg)) => {
                        if let Err(err) = self.create_dir(&sess, &msg) {
                            log::error!("{:?} -> error: {:#}", msg, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::RemoveDir(msg)) => {
                        if let Err(err) = self.remove_dir(&sess, &msg) {
                            log::error!("{:?} -> error: {:#}", msg, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::Metadata(msg)) => {
                        if let Err(err) = self.metadata(&sess, &msg) {
                            log::error!("{:?} -> error: {:#}", msg, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::SymlinkMetadata(msg)) => {
                        if let Err(err) = self.symlink_metadata(&sess, &msg) {
                            log::error!("{:?} -> error: {:#}", msg, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::SetMetadata(msg)) => {
                        if let Err(err) = self.set_metadata(&sess, &msg) {
                            log::error!("{:?} -> error: {:#}", msg, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::Symlink(msg)) => {
                        if let Err(err) = self.symlink(&sess, &msg) {
                            log::error!("{:?} -> error: {:#}", msg, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::ReadLink(msg)) => {
                        if let Err(err) = self.read_link(&sess, &msg) {
                            log::error!("{:?} -> error: {:#}", msg, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::Canonicalize(msg)) => {
                        if let Err(err) = self.canonicalize(&sess, &msg) {
                            log::error!("{:?} -> error: {:#}", msg, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::Rename(msg)) => {
                        if let Err(err) = self.rename(&sess, &msg) {
                            log::error!("{:?} -> error: {:#}", msg, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::RemoveFile(msg)) => {
                        if let Err(err) = self.remove_file(&sess, &msg) {
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

    pub fn exec(&mut self, sess: &ssh2::Session, exec: &Exec) -> anyhow::Result<()> {
        sess.set_blocking(true);

        let mut channel = sess.channel_session()?;

        if let Some(env) = &exec.env {
            for (key, val) in env {
                if let Err(err) = channel.setenv(key, val) {
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

        channel.exec(&exec.command_line)?;

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
    ///
    /// See [`Sftp::open_mode`] for more information.
    pub fn open_with_mode(
        &mut self,
        sess: &ssh2::Session,
        msg: &sftp::OpenWithMode,
    ) -> anyhow::Result<()> {
        let flags: ssh2::OpenFlags = msg.opts.into();
        let mode = msg.opts.mode;
        let open_type: ssh2::OpenType = msg.opts.ty.into();

        let result = self.init_sftp(sess).and_then(|sftp| {
            sftp.open_mode(msg.filename.as_path(), flags, mode, open_type)
                .map_err(SftpChannelError::from)
        });

        match result {
            Ok(ssh_file) => {
                let (file_id, file) = self.make_file();
                msg.reply.try_send(Ok(file))?;
                self.files.insert(file_id, ssh_file);
            }
            Err(x) => msg.reply.try_send(Err(x))?,
        }

        Ok(())
    }

    /// Helper to open a file in the `Read` mode.
    ///
    /// See [`Sftp::open`] for more information.
    pub fn open(&mut self, sess: &ssh2::Session, msg: &sftp::Open) -> anyhow::Result<()> {
        let result = self.init_sftp(sess).and_then(|sftp| {
            sftp.open(msg.filename.as_path())
                .map_err(SftpChannelError::from)
        });

        match result {
            Ok(ssh_file) => {
                let (file_id, file) = self.make_file();
                msg.reply.try_send(Ok(file))?;
                self.files.insert(file_id, ssh_file);
            }
            Err(x) => msg.reply.try_send(Err(x))?,
        }

        Ok(())
    }

    /// Helper to create a file in write-only mode with truncation.
    ///
    /// See [`Sftp::create`] for more information.
    pub fn create(&mut self, sess: &ssh2::Session, msg: &sftp::Create) -> anyhow::Result<()> {
        let result = self.init_sftp(sess).and_then(|sftp| {
            sftp.create(msg.filename.as_path())
                .map_err(SftpChannelError::from)
        });

        match result {
            Ok(ssh_file) => {
                let (file_id, file) = self.make_file();
                msg.reply.try_send(Ok(file))?;
                self.files.insert(file_id, ssh_file);
            }
            Err(x) => msg.reply.try_send(Err(x))?,
        }

        Ok(())
    }

    /// Helper to open a directory for reading its contents.
    ///
    /// See [`Sftp::opendir`] for more information.
    pub fn open_dir(&mut self, sess: &ssh2::Session, msg: &sftp::OpenDir) -> anyhow::Result<()> {
        let result = self.init_sftp(sess).and_then(|sftp| {
            sftp.opendir(msg.filename.as_path())
                .map_err(SftpChannelError::from)
        });

        match result {
            Ok(ssh_file) => {
                let (file_id, file) = self.make_file();
                msg.reply.try_send(Ok(file))?;
                self.files.insert(file_id, ssh_file);
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
    fn write_file(&mut self, _sess: &ssh2::Session, msg: &sftp::WriteFile) -> anyhow::Result<()> {
        let sftp::WriteFile {
            file_id,
            data,
            reply,
        } = msg;

        if let Some(file) = self.files.get_mut(file_id) {
            let result = file.write_all(data).map_err(SftpChannelError::from);
            reply.try_send(result)?;
        }

        Ok(())
    }

    /// Reads from a loaded file.
    fn read_file(&mut self, _sess: &ssh2::Session, msg: &sftp::ReadFile) -> anyhow::Result<()> {
        let sftp::ReadFile {
            file_id,
            max_bytes,
            reply,
        } = msg;

        if let Some(file) = self.files.get_mut(file_id) {
            // TODO: Move this somewhere to avoid re-allocating buffer
            let mut buf = vec![0u8; *max_bytes];
            match file.read(&mut buf).map_err(SftpChannelError::from) {
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
    fn close_file(&mut self, _sess: &ssh2::Session, msg: &sftp::CloseFile) -> anyhow::Result<()> {
        self.files.remove(&msg.file_id);
        msg.reply.try_send(Ok(()))?;

        Ok(())
    }

    /// Flushes a file.
    fn flush_file(&mut self, _sess: &ssh2::Session, msg: &sftp::FlushFile) -> anyhow::Result<()> {
        if let Some(file) = self.files.get_mut(&msg.file_id) {
            let result = file.flush().map_err(SftpChannelError::from);
            msg.reply.try_send(result)?;
        }

        Ok(())
    }

    /// Sets file metadata.
    fn set_metadata_file(
        &mut self,
        _sess: &ssh2::Session,
        msg: &sftp::SetMetadataFile,
    ) -> anyhow::Result<()> {
        let sftp::SetMetadataFile {
            file_id,
            metadata,
            reply,
        } = msg;

        if let Some(file) = self.files.get_mut(file_id) {
            let result = file
                .setstat((*metadata).into())
                .map_err(SftpChannelError::from);
            reply.try_send(result)?;
        }

        Ok(())
    }

    /// Gets file stat.
    fn metadata_file(
        &mut self,
        _sess: &ssh2::Session,
        msg: &sftp::MetadataFile,
    ) -> anyhow::Result<()> {
        if let Some(file) = self.files.get_mut(&msg.file_id) {
            let result = file
                .stat()
                .map(Metadata::from)
                .map_err(SftpChannelError::from);
            msg.reply.try_send(result)?;
        }

        Ok(())
    }

    /// Performs readdir for file.
    fn read_dir_file(
        &mut self,
        _sess: &ssh2::Session,
        msg: &sftp::ReadDirFile,
    ) -> anyhow::Result<()> {
        if let Some(file) = self.files.get_mut(&msg.file_id) {
            let result = file
                .readdir()
                .map(|(path, stat)| (path, Metadata::from(stat)))
                .map_err(SftpChannelError::from);
            msg.reply.try_send(result)?;
        }

        Ok(())
    }

    /// Fsync file.
    fn fsync_file(
        &mut self,
        _sess: &ssh2::Session,
        fsync_file: &sftp::FsyncFile,
    ) -> anyhow::Result<()> {
        if let Some(file) = self.files.get_mut(&fsync_file.file_id) {
            let result = file.fsync().map_err(SftpChannelError::from);
            fsync_file.reply.try_send(result)?;
        }

        Ok(())
    }

    /// Convenience function to read the files in a directory.
    ///
    /// See [`Sftp::readdir`] for more information.
    pub fn read_dir(&mut self, sess: &ssh2::Session, msg: &sftp::ReadDir) -> anyhow::Result<()> {
        let result = self.init_sftp(sess).and_then(|sftp| {
            sftp.readdir(msg.filename.as_path())
                .map(|entries| {
                    entries
                        .into_iter()
                        .map(|(path, stat)| (path, Metadata::from(stat)))
                        .collect()
                })
                .map_err(SftpChannelError::from)
        });
        msg.reply.try_send(result)?;

        Ok(())
    }

    /// Create a directory on the remote filesystem.
    ///
    /// See [`Sftp::rmdir`] for more information.
    pub fn create_dir(
        &mut self,
        sess: &ssh2::Session,
        msg: &sftp::CreateDir,
    ) -> anyhow::Result<()> {
        let result = self.init_sftp(sess).and_then(|sftp| {
            sftp.mkdir(msg.filename.as_path(), msg.mode)
                .map_err(SftpChannelError::from)
        });
        msg.reply.try_send(result)?;

        Ok(())
    }

    /// Remove a directory from the remote filesystem.
    ///
    /// See [`Sftp::rmdir`] for more information.
    pub fn remove_dir(
        &mut self,
        sess: &ssh2::Session,
        msg: &sftp::RemoveDir,
    ) -> anyhow::Result<()> {
        let result = self.init_sftp(sess).and_then(|sftp| {
            sftp.rmdir(msg.filename.as_path())
                .map_err(SftpChannelError::from)
        });
        msg.reply.try_send(result)?;

        Ok(())
    }

    /// Get the metadata for a file, performed by stat(2).
    ///
    /// See [`Sftp::stat`] for more information.
    pub fn metadata(
        &mut self,
        sess: &ssh2::Session,
        msg: &sftp::GetMetadata,
    ) -> anyhow::Result<()> {
        let result = self.init_sftp(sess).and_then(|sftp| {
            sftp.stat(msg.filename.as_path())
                .map(Metadata::from)
                .map_err(SftpChannelError::from)
        });
        msg.reply.try_send(result)?;

        Ok(())
    }

    /// Get the metadata for a file, performed by lstat(2).
    ///
    /// See [`Sftp::lstat`] for more information.
    pub fn symlink_metadata(
        &mut self,
        sess: &ssh2::Session,
        msg: &sftp::SymlinkMetadata,
    ) -> anyhow::Result<()> {
        let result = self.init_sftp(sess).and_then(|sftp| {
            sftp.lstat(msg.filename.as_path())
                .map(Metadata::from)
                .map_err(SftpChannelError::from)
        });
        msg.reply.try_send(result)?;

        Ok(())
    }

    /// Set the metadata for a file.
    ///
    /// See [`Sftp::setstat`] for more information.
    pub fn set_metadata(
        &mut self,
        sess: &ssh2::Session,
        msg: &sftp::SetMetadata,
    ) -> anyhow::Result<()> {
        let result = self.init_sftp(sess).and_then(|sftp| {
            sftp.setstat(msg.filename.as_path(), msg.metadata.into())
                .map_err(SftpChannelError::from)
        });
        msg.reply.try_send(result)?;

        Ok(())
    }

    /// Create symlink at `target` pointing at `path`.
    ///
    /// See [`Sftp::symlink`] for more information.
    pub fn symlink(&mut self, sess: &ssh2::Session, msg: &sftp::Symlink) -> anyhow::Result<()> {
        let result = self.init_sftp(sess).and_then(|sftp| {
            sftp.symlink(msg.path.as_path(), msg.target.as_path())
                .map_err(SftpChannelError::from)
        });
        msg.reply.try_send(result)?;

        Ok(())
    }

    /// Read a symlink at `path`.
    ///
    /// See [`Sftp::readlink`] for more information.
    pub fn read_link(&mut self, sess: &ssh2::Session, msg: &sftp::ReadLink) -> anyhow::Result<()> {
        let result = self.init_sftp(sess).and_then(|sftp| {
            sftp.readlink(msg.path.as_path())
                .map_err(SftpChannelError::from)
        });
        msg.reply.try_send(result)?;

        Ok(())
    }

    /// Resolve the real path for `path`.
    ///
    /// See [`Sftp::realpath`] for more information.
    pub fn canonicalize(
        &mut self,
        sess: &ssh2::Session,
        msg: &sftp::Canonicalize,
    ) -> anyhow::Result<()> {
        let result = self.init_sftp(sess).and_then(|sftp| {
            sftp.realpath(msg.path.as_path())
                .map_err(SftpChannelError::from)
        });
        msg.reply.try_send(result)?;

        Ok(())
    }

    /// Rename the filesystem object on the remote filesystem.
    ///
    /// See [`Sftp::rename`] for more information.
    pub fn rename(&mut self, sess: &ssh2::Session, msg: &sftp::Rename) -> anyhow::Result<()> {
        let result = self.init_sftp(sess).and_then(|sftp| {
            sftp.rename(msg.src.as_path(), msg.dst.as_path(), Some(msg.opts.into()))
                .map_err(SftpChannelError::from)
        });
        msg.reply.try_send(result)?;

        Ok(())
    }

    /// Remove a file on the remote filesystem.
    ///
    /// See [`Sftp::unlink`] for more information.
    pub fn remove_file(
        &mut self,
        sess: &ssh2::Session,
        msg: &sftp::RemoveFile,
    ) -> anyhow::Result<()> {
        let result = self.init_sftp(sess).and_then(|sftp| {
            sftp.unlink(msg.file.as_path())
                .map_err(SftpChannelError::from)
        });
        msg.reply.try_send(result)?;

        Ok(())
    }

    /// Initialize the sftp channel if not already created, returning a mutable reference to it
    fn init_sftp<'a, 'b>(
        &'a mut self,
        sess: &'b ssh2::Session,
    ) -> SftpChannelResult<&'a mut ssh2::Sftp> {
        if self.sftp.is_none() {
            let blocking = sess.is_blocking();
            sess.set_blocking(true);

            self.sftp = Some(sess.sftp()?);

            sess.set_blocking(blocking);
        }

        // NOTE: sftp should have been replaced with Some(sftp) from above
        Ok(self.sftp.as_mut().unwrap())
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
            sftp: None,
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
