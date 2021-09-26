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
    WriteMode,
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
                    SessionRequest::Sftp(SftpRequest::OpenMode(open_mode)) => {
                        if let Err(err) = self.open_mode(&sess, &open_mode) {
                            log::error!("{:?} -> error: {:#}", open_mode, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::Open(open)) => {
                        if let Err(err) = self.open(&sess, &open) {
                            log::error!("{:?} -> error: {:#}", open, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::Create(create)) => {
                        if let Err(err) = self.create(&sess, &create) {
                            log::error!("{:?} -> error: {:#}", create, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::Opendir(opendir)) => {
                        if let Err(err) = self.opendir(&sess, &opendir) {
                            log::error!("{:?} -> error: {:#}", opendir, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::File(FileRequest::Write(write_file))) => {
                        if let Err(err) = self.write_file(&sess, &write_file) {
                            log::error!("{:?} -> error: {:#}", write_file, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::File(FileRequest::Read(read_file))) => {
                        if let Err(err) = self.read_file(&sess, &read_file) {
                            log::error!("{:?} -> error: {:#}", read_file, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::File(FileRequest::Close(close_file))) => {
                        if let Err(err) = self.close_file(&sess, &close_file) {
                            log::error!("{:?} -> error: {:#}", close_file, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::File(FileRequest::Flush(flush_file))) => {
                        if let Err(err) = self.flush_file(&sess, &flush_file) {
                            log::error!("{:?} -> error: {:#}", flush_file, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::File(FileRequest::Setstat(setstat_file))) => {
                        if let Err(err) = self.setstat_file(&sess, &setstat_file) {
                            log::error!("{:?} -> error: {:#}", setstat_file, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::File(FileRequest::Stat(stat_file))) => {
                        if let Err(err) = self.stat_file(&sess, &stat_file) {
                            log::error!("{:?} -> error: {:#}", stat_file, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::File(FileRequest::Readdir(readdir_file))) => {
                        if let Err(err) = self.readdir_file(&sess, &readdir_file) {
                            log::error!("{:?} -> error: {:#}", readdir_file, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::File(FileRequest::Fsync(fsync_file))) => {
                        if let Err(err) = self.fsync_file(&sess, &fsync_file) {
                            log::error!("{:?} -> error: {:#}", fsync_file, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::Readdir(readdir)) => {
                        if let Err(err) = self.readdir(&sess, &readdir) {
                            log::error!("{:?} -> error: {:#}", readdir, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::Mkdir(mkdir)) => {
                        if let Err(err) = self.mkdir(&sess, &mkdir) {
                            log::error!("{:?} -> error: {:#}", mkdir, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::Rmdir(rmdir)) => {
                        if let Err(err) = self.rmdir(&sess, &rmdir) {
                            log::error!("{:?} -> error: {:#}", rmdir, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::Stat(stat)) => {
                        if let Err(err) = self.stat(&sess, &stat) {
                            log::error!("{:?} -> error: {:#}", stat, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::Lstat(lstat)) => {
                        if let Err(err) = self.lstat(&sess, &lstat) {
                            log::error!("{:?} -> error: {:#}", lstat, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::Setstat(setstat)) => {
                        if let Err(err) = self.setstat(&sess, &setstat) {
                            log::error!("{:?} -> error: {:#}", setstat, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::Symlink(symlink)) => {
                        if let Err(err) = self.symlink(&sess, &symlink) {
                            log::error!("{:?} -> error: {:#}", symlink, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::Readlink(readlink)) => {
                        if let Err(err) = self.readlink(&sess, &readlink) {
                            log::error!("{:?} -> error: {:#}", readlink, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::Realpath(realpath)) => {
                        if let Err(err) = self.realpath(&sess, &realpath) {
                            log::error!("{:?} -> error: {:#}", realpath, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::Rename(rename)) => {
                        if let Err(err) = self.rename(&sess, &rename) {
                            log::error!("{:?} -> error: {:#}", rename, err);
                        }
                        Ok(true)
                    }
                    SessionRequest::Sftp(SftpRequest::Unlink(unlink)) => {
                        if let Err(err) = self.unlink(&sess, &unlink) {
                            log::error!("{:?} -> error: {:#}", unlink, err);
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
    pub fn open_mode(
        &mut self,
        sess: &ssh2::Session,
        open_mode: &sftp::OpenMode,
    ) -> anyhow::Result<()> {
        let flags: ssh2::OpenFlags = open_mode.opts.into();
        let mode = open_mode.opts.mode;
        let open_type: ssh2::OpenType = open_mode.opts.ty.into();

        let ssh_file = self.init_sftp(sess)?.open_mode(
            open_mode.filename.as_path(),
            flags,
            mode,
            open_type,
        )?;

        let (file_id, file) = self.make_file();
        open_mode.reply.try_send(file)?;
        self.files.insert(file_id, ssh_file);

        Ok(())
    }

    /// Helper to open a file in the `Read` mode.
    ///
    /// See [`Sftp::open`] for more information.
    pub fn open(&mut self, sess: &ssh2::Session, open: &sftp::Open) -> anyhow::Result<()> {
        let ssh_file = self.init_sftp(sess)?.open(open.filename.as_path())?;

        let (file_id, file) = self.make_file();
        open.reply.try_send(file)?;
        self.files.insert(file_id, ssh_file);

        Ok(())
    }

    /// Helper to create a file in write-only mode with truncation.
    ///
    /// See [`Sftp::create`] for more information.
    pub fn create(&mut self, sess: &ssh2::Session, create: &sftp::Create) -> anyhow::Result<()> {
        let ssh_file = self.init_sftp(sess)?.create(create.filename.as_path())?;

        let (file_id, file) = self.make_file();
        create.reply.try_send(file)?;
        self.files.insert(file_id, ssh_file);

        Ok(())
    }

    /// Helper to open a directory for reading its contents.
    ///
    /// See [`Sftp::opendir`] for more information.
    pub fn opendir(&mut self, sess: &ssh2::Session, opendir: &sftp::Opendir) -> anyhow::Result<()> {
        let ssh_file = self.init_sftp(sess)?.opendir(opendir.filename.as_path())?;

        let (file_id, file) = self.make_file();
        opendir.reply.try_send(file)?;
        self.files.insert(file_id, ssh_file);

        Ok(())
    }

    fn make_file(&mut self) -> (FileId, File) {
        let file_id = self.next_file_id;
        self.next_file_id += 1;

        let file = File::new(file_id);
        (file_id, file)
    }

    /// Writes to a loaded file.
    fn write_file(
        &mut self,
        _sess: &ssh2::Session,
        write_file: &sftp::WriteFile,
    ) -> anyhow::Result<()> {
        let sftp::WriteFile {
            file_id,
            data,
            reply,
        } = write_file;

        if let Some(file) = self.files.get_mut(file_id) {
            file.write_all(data)?;
        }
        reply.try_send(())?;

        Ok(())
    }

    /// Reads from a loaded file.
    fn read_file(
        &mut self,
        _sess: &ssh2::Session,
        read_file: &sftp::ReadFile,
    ) -> anyhow::Result<()> {
        let sftp::ReadFile {
            file_id,
            max_bytes,
            reply,
        } = read_file;

        if let Some(file) = self.files.get_mut(file_id) {
            // TODO: Move this somewhere to avoid re-allocating buffer
            let mut buf = vec![0u8; *max_bytes];
            let n = file.read(&mut buf)?;
            buf.truncate(n);
            reply.try_send(buf)?;
        } else {
            reply.try_send(Vec::new())?;
        }

        Ok(())
    }

    /// Closes a file and removes it from the internal memory.
    fn close_file(
        &mut self,
        _sess: &ssh2::Session,
        close_file: &sftp::CloseFile,
    ) -> anyhow::Result<()> {
        self.files.remove(&close_file.file_id);
        close_file.reply.try_send(())?;

        Ok(())
    }

    /// Flushes a file.
    fn flush_file(
        &mut self,
        _sess: &ssh2::Session,
        flush_file: &sftp::FlushFile,
    ) -> anyhow::Result<()> {
        if let Some(file) = self.files.get_mut(&flush_file.file_id) {
            file.flush()?;
        }
        flush_file.reply.try_send(())?;

        Ok(())
    }

    /// Sets file stat.
    fn setstat_file(
        &mut self,
        _sess: &ssh2::Session,
        setstat_file: &sftp::SetstatFile,
    ) -> anyhow::Result<()> {
        let sftp::SetstatFile {
            file_id,
            metadata,
            reply,
        } = setstat_file;

        if let Some(file) = self.files.get_mut(file_id) {
            file.setstat((*metadata).into())?;
        }
        reply.try_send(())?;

        Ok(())
    }

    /// Gets file stat.
    fn stat_file(
        &mut self,
        _sess: &ssh2::Session,
        stat_file: &sftp::StatFile,
    ) -> anyhow::Result<()> {
        if let Some(file) = self.files.get_mut(&stat_file.file_id) {
            let stat = file.stat()?;
            stat_file.reply.try_send(Metadata::from(stat))?;
        }

        Ok(())
    }

    /// Performs readdir for file.
    fn readdir_file(
        &mut self,
        _sess: &ssh2::Session,
        readdir_file: &sftp::ReaddirFile,
    ) -> anyhow::Result<()> {
        if let Some(file) = self.files.get_mut(&readdir_file.file_id) {
            let (path, stat) = file.readdir()?;
            readdir_file.reply.try_send((path, Metadata::from(stat)))?;
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
            file.fsync()?;
        }
        fsync_file.reply.try_send(())?;

        Ok(())
    }

    /// Convenience function to read the files in a directory.
    ///
    /// See [`Sftp::readdir`] for more information.
    pub fn readdir(&mut self, sess: &ssh2::Session, readdir: &sftp::Readdir) -> anyhow::Result<()> {
        let result = self
            .init_sftp(sess)?
            .readdir(readdir.filename.as_path())?
            .into_iter()
            .map(|(path, stat)| (path, Metadata::from(stat)))
            .collect();
        readdir.reply.try_send(result)?;

        Ok(())
    }

    /// Create a directory on the remote filesystem.
    ///
    /// See [`Sftp::rmdir`] for more information.
    pub fn mkdir(&mut self, sess: &ssh2::Session, mkdir: &sftp::Mkdir) -> anyhow::Result<()> {
        self.init_sftp(sess)?
            .mkdir(mkdir.filename.as_path(), mkdir.mode)?;
        mkdir.reply.try_send(())?;

        Ok(())
    }

    /// Remove a directory from the remote filesystem.
    ///
    /// See [`Sftp::rmdir`] for more information.
    pub fn rmdir(&mut self, sess: &ssh2::Session, rmdir: &sftp::Rmdir) -> anyhow::Result<()> {
        self.init_sftp(sess)?.rmdir(rmdir.filename.as_path())?;
        rmdir.reply.try_send(())?;

        Ok(())
    }

    /// Get the metadata for a file, performed by stat(2).
    ///
    /// See [`Sftp::stat`] for more information.
    pub fn stat(&mut self, sess: &ssh2::Session, stat: &sftp::Stat) -> anyhow::Result<()> {
        let metadata = Metadata::from(self.init_sftp(sess)?.stat(stat.filename.as_path())?);
        stat.reply.try_send(metadata)?;

        Ok(())
    }

    /// Get the metadata for a file, performed by lstat(2).
    ///
    /// See [`Sftp::lstat`] for more information.
    pub fn lstat(&mut self, sess: &ssh2::Session, lstat: &sftp::Lstat) -> anyhow::Result<()> {
        let metadata = Metadata::from(self.init_sftp(sess)?.lstat(lstat.filename.as_path())?);
        lstat.reply.try_send(metadata)?;

        Ok(())
    }

    /// Set the metadata for a file.
    ///
    /// See [`Sftp::setstat`] for more information.
    pub fn setstat(&mut self, sess: &ssh2::Session, setstat: &sftp::Setstat) -> anyhow::Result<()> {
        self.init_sftp(sess)?
            .setstat(setstat.filename.as_path(), setstat.metadata.into())?;
        setstat.reply.try_send(())?;

        Ok(())
    }

    /// Create symlink at `target` pointing at `path`.
    ///
    /// See [`Sftp::symlink`] for more information.
    pub fn symlink(&mut self, sess: &ssh2::Session, symlink: &sftp::Symlink) -> anyhow::Result<()> {
        self.init_sftp(sess)?
            .symlink(symlink.path.as_path(), symlink.target.as_path())?;
        symlink.reply.try_send(())?;

        Ok(())
    }

    /// Read a symlink at `path`.
    ///
    /// See [`Sftp::readlink`] for more information.
    pub fn readlink(
        &mut self,
        sess: &ssh2::Session,
        readlink: &sftp::Readlink,
    ) -> anyhow::Result<()> {
        let path = self.init_sftp(sess)?.readlink(readlink.path.as_path())?;
        readlink.reply.try_send(path)?;

        Ok(())
    }

    /// Resolve the real path for `path`.
    ///
    /// See [`Sftp::realpath`] for more information.
    pub fn realpath(
        &mut self,
        sess: &ssh2::Session,
        realpath: &sftp::Realpath,
    ) -> anyhow::Result<()> {
        let path = self.init_sftp(sess)?.realpath(realpath.path.as_path())?;
        realpath.reply.try_send(path)?;

        Ok(())
    }

    /// Rename the filesystem object on the remote filesystem.
    ///
    /// See [`Sftp::rename`] for more information.
    pub fn rename(&mut self, sess: &ssh2::Session, rename: &sftp::Rename) -> anyhow::Result<()> {
        self.init_sftp(sess)?.rename(
            rename.src.as_path(),
            rename.dst.as_path(),
            Some(rename.opts.into()),
        )?;
        rename.reply.try_send(())?;

        Ok(())
    }

    /// Remove a file on the remote filesystem.
    ///
    /// See [`Sftp::unlink`] for more information.
    pub fn unlink(&mut self, sess: &ssh2::Session, unlink: &sftp::Unlink) -> anyhow::Result<()> {
        self.init_sftp(sess)?.unlink(unlink.file.as_path())?;
        unlink.reply.try_send(())?;

        Ok(())
    }

    /// Initialize the sftp channel if not already created, returning a mutable reference to it
    fn init_sftp<'a, 'b>(
        &'a mut self,
        sess: &'b ssh2::Session,
    ) -> anyhow::Result<&'a mut ssh2::Sftp> {
        if self.sftp.is_none() {
            self.do_blocking(sess, |this, sess| {
                this.sftp = Some(sess.sftp()?);
                Ok(())
            })?;
        }

        // NOTE: sftp should have been replaced with Some(sftp) from above
        Ok(self.sftp.as_mut().unwrap())
    }

    fn do_blocking<F, R>(&mut self, sess: &ssh2::Session, mut f: F) -> anyhow::Result<R>
    where
        F: FnMut(&mut Self, &ssh2::Session) -> anyhow::Result<R>,
    {
        let blocking = sess.is_blocking();
        sess.set_blocking(true);
        let result = f(self, sess);
        sess.set_blocking(blocking);
        result
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
