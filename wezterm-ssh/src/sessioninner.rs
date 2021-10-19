use crate::channelwrap::ChannelWrap;
use crate::config::ConfigMap;
use crate::dirwrap::DirWrap;
use crate::filewrap::FileWrap;
use crate::pty::*;
use crate::session::{Exec, ExecResult, SessionEvent, SessionRequest, SignalChannel};
use crate::sessionwrap::SessionWrap;
use crate::sftp::dir::{CloseDir, Dir, DirId, DirRequest, ReadDirHandle};
use crate::sftp::file::{
    CloseFile, File, FileId, FileRequest, FlushFile, FsyncFile, MetadataFile, ReadFile,
    SetMetadataFile, WriteFile,
};
use crate::sftp::{
    Canonicalize, CreateDir, GetMetadata, OpenDir, OpenWithMode, ReadDir, ReadLink, RemoveDir,
    RemoveFile, Rename, SetMetadata, SftpChannelError, SftpChannelResult, SftpRequest, Symlink,
    SymlinkMetadata,
};
use crate::sftpwrap::SftpWrap;
use anyhow::{anyhow, Context};
use filedescriptor::{
    poll, pollfd, socketpair, AsRawSocketDescriptor, FileDescriptor, POLLIN, POLLOUT,
};
use libssh_rs as libssh;
use portable_pty::ExitStatus;
use smol::channel::{bounded, Receiver, Sender, TryRecvError};
use std::collections::{HashMap, VecDeque};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

#[derive(Debug)]
pub(crate) struct DescriptorState {
    pub fd: Option<FileDescriptor>,
    pub buf: VecDeque<u8>,
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
    pub dirs: HashMap<DirId, DirWrap>,
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
    pub fn run(&mut self) {
        if let Err(err) = self.run_impl() {
            self.tx_event
                .try_send(SessionEvent::Error(format!("{:#}", err)))
                .ok();
        }
    }

    fn run_impl(&mut self) -> anyhow::Result<()> {
        let backend = self
            .config
            .get("wezterm_ssh_backend")
            .map(|s| s.as_str())
            .unwrap_or("ssh2");
        match backend {
            "ssh2" => self.run_impl_ssh2(),
            "libssh" => self.run_impl_libssh(),
            _ => anyhow::bail!(
                "invalid wezterm_ssh_backend value: {}, expected either `ssh2` or `libssh`"
            ),
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

        self.tx_event
            .try_send(SessionEvent::Banner(Some(format!(
                "Using libssh-rs to connect to {}@{}:{}",
                user, hostname, port
            ))))
            .context("notifying user of banner")?;

        let sess = libssh::Session::new()?;
        // sess.set_option(libssh::SshOption::LogLevel(libssh::LogLevel::Packet))?;
        sess.set_option(libssh::SshOption::Hostname(hostname.clone()))?;
        sess.set_option(libssh::SshOption::User(Some(user)))?;
        sess.set_option(libssh::SshOption::Port(port))?;
        if let Some(files) = self.config.get("identityfile") {
            for file in files.split_whitespace() {
                sess.set_option(libssh::SshOption::AddIdentity(file.to_string()))?;
            }
        }
        if let Some(kh) = self.config.get("userknownhostsfile") {
            sess.set_option(libssh::SshOption::KnownHosts(Some(kh.to_string())))?;
        }

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

        self.tx_event
            .try_send(SessionEvent::Banner(Some(format!(
                "Using ssh2 to connect to {}@{}:{}",
                user, hostname, port
            ))))
            .context("notifying user of banner")?;

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
                            log::trace!(
                                "Failed to read data from channel: {:#}, but \
                                         still have some buffer to drain",
                                err
                            );
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
                    SessionRequest::Sftp(SftpRequest::Dir(DirRequest::Close(msg))) => {
                        if let Err(err) = self.close_dir(sess, &msg) {
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
                    SessionRequest::Sftp(SftpRequest::Dir(DirRequest::ReadDir(msg))) => {
                        if let Err(err) = self.read_dir_handle(sess, &msg) {
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
        msg: &OpenWithMode,
    ) -> anyhow::Result<()> {
        let result = self
            .init_sftp(sess)
            .and_then(|sftp| sftp.open(&msg.filename, msg.opts));

        match result {
            Ok(ssh_file) => {
                let file_id = self.next_file_id;
                self.next_file_id += 1;

                let file = File::new(file_id);

                msg.reply.try_send(Ok(file))?;
                self.files.insert(file_id, ssh_file);
            }
            Err(x) => msg.reply.try_send(Err(x))?,
        }

        Ok(())
    }

    /// Helper to open a directory for reading its contents.
    pub fn open_dir(&mut self, sess: &mut SessionWrap, msg: &OpenDir) -> anyhow::Result<()> {
        let result = self
            .init_sftp(sess)
            .and_then(|sftp| sftp.open_dir(&msg.filename));

        match result {
            Ok(ssh_file) => {
                let dir_id = self.next_file_id;
                self.next_file_id += 1;

                let dir = Dir::new(dir_id);

                msg.reply.try_send(Ok(dir))?;
                self.dirs.insert(dir_id, ssh_file);
            }
            Err(x) => msg.reply.try_send(Err(x))?,
        }

        Ok(())
    }

    /// Writes to a loaded file.
    fn write_file(&mut self, _sess: &mut SessionWrap, msg: &WriteFile) -> anyhow::Result<()> {
        let WriteFile {
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
    fn read_file(&mut self, _sess: &mut SessionWrap, msg: &ReadFile) -> anyhow::Result<()> {
        let ReadFile {
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

    fn close_dir(&mut self, _sess: &mut SessionWrap, msg: &CloseDir) -> anyhow::Result<()> {
        self.dirs.remove(&msg.dir_id);
        msg.reply.try_send(Ok(()))?;

        Ok(())
    }

    /// Closes a file and removes it from the internal memory.
    fn close_file(&mut self, _sess: &mut SessionWrap, msg: &CloseFile) -> anyhow::Result<()> {
        self.files.remove(&msg.file_id);
        msg.reply.try_send(Ok(()))?;

        Ok(())
    }

    /// Flushes a file.
    fn flush_file(&mut self, _sess: &mut SessionWrap, msg: &FlushFile) -> anyhow::Result<()> {
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
        msg: &SetMetadataFile,
    ) -> anyhow::Result<()> {
        let SetMetadataFile {
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
    fn metadata_file(&mut self, _sess: &mut SessionWrap, msg: &MetadataFile) -> anyhow::Result<()> {
        if let Some(file) = self.files.get_mut(&msg.file_id) {
            let result = file.metadata();
            msg.reply.try_send(result)?;
        }

        Ok(())
    }

    /// Performs readdir for file.
    fn read_dir_handle(
        &mut self,
        _sess: &mut SessionWrap,
        msg: &ReadDirHandle,
    ) -> anyhow::Result<()> {
        if let Some(dir) = self.dirs.get_mut(&msg.dir_id) {
            let result = dir.read_dir();
            msg.reply.try_send(result)?;
        }

        Ok(())
    }

    /// Fsync file.
    fn fsync_file(
        &mut self,
        _sess: &mut SessionWrap,
        fsync_file: &FsyncFile,
    ) -> anyhow::Result<()> {
        if let Some(file) = self.files.get_mut(&fsync_file.file_id) {
            let result = file.fsync();
            fsync_file.reply.try_send(result)?;
        }

        Ok(())
    }

    /// Convenience function to read the files in a directory.
    pub fn read_dir(&mut self, sess: &mut SessionWrap, msg: &ReadDir) -> anyhow::Result<()> {
        let result = self
            .init_sftp(sess)
            .and_then(|sftp| sftp.read_dir(&msg.filename));
        msg.reply.try_send(result)?;

        Ok(())
    }

    /// Create a directory on the remote filesystem.
    pub fn create_dir(&mut self, sess: &mut SessionWrap, msg: &CreateDir) -> anyhow::Result<()> {
        let result = self
            .init_sftp(sess)
            .and_then(|sftp| sftp.create_dir(&msg.filename, msg.mode));
        msg.reply.try_send(result)?;

        Ok(())
    }

    /// Remove a directory from the remote filesystem.
    pub fn remove_dir(&mut self, sess: &mut SessionWrap, msg: &RemoveDir) -> anyhow::Result<()> {
        let result = self
            .init_sftp(sess)
            .and_then(|sftp| sftp.remove_dir(&msg.filename));
        msg.reply.try_send(result)?;

        Ok(())
    }

    /// Get the metadata for a file, performed by stat(2).
    pub fn metadata(&mut self, sess: &mut SessionWrap, msg: &GetMetadata) -> anyhow::Result<()> {
        let result = self
            .init_sftp(sess)
            .and_then(|sftp| sftp.metadata(&msg.filename));
        msg.reply.try_send(result)?;

        Ok(())
    }

    /// Get the metadata for a file, performed by lstat(2).
    pub fn symlink_metadata(
        &mut self,
        sess: &mut SessionWrap,
        msg: &SymlinkMetadata,
    ) -> anyhow::Result<()> {
        let result = self
            .init_sftp(sess)
            .and_then(|sftp| sftp.symlink_metadata(&msg.filename));
        msg.reply.try_send(result)?;

        Ok(())
    }

    /// Set the metadata for a file.
    pub fn set_metadata(
        &mut self,
        sess: &mut SessionWrap,
        msg: &SetMetadata,
    ) -> anyhow::Result<()> {
        let result = self
            .init_sftp(sess)
            .and_then(|sftp| sftp.set_metadata(&msg.filename, msg.metadata));
        msg.reply.try_send(result)?;

        Ok(())
    }

    /// Create symlink at `target` pointing at `path`.
    pub fn symlink(&mut self, sess: &mut SessionWrap, msg: &Symlink) -> anyhow::Result<()> {
        let result = self
            .init_sftp(sess)
            .and_then(|sftp| sftp.symlink(&msg.path, &msg.target));
        msg.reply.try_send(result)?;

        Ok(())
    }

    /// Read a symlink at `path`.
    pub fn read_link(&mut self, sess: &mut SessionWrap, msg: &ReadLink) -> anyhow::Result<()> {
        let result = self
            .init_sftp(sess)
            .and_then(|sftp| sftp.read_link(&msg.path));
        msg.reply.try_send(result)?;

        Ok(())
    }

    /// Resolve the real path for `path`.
    pub fn canonicalize(
        &mut self,
        sess: &mut SessionWrap,
        msg: &Canonicalize,
    ) -> anyhow::Result<()> {
        let result = self
            .init_sftp(sess)
            .and_then(|sftp| sftp.canonicalize(&msg.path));
        msg.reply.try_send(result)?;

        Ok(())
    }

    /// Rename the filesystem object on the remote filesystem.
    pub fn rename(&mut self, sess: &mut SessionWrap, msg: &Rename) -> anyhow::Result<()> {
        let result = self.init_sftp(sess).and_then(|sftp| {
            sftp.rename(&msg.src, &msg.dst, msg.opts)
                .map_err(SftpChannelError::from)
        });
        msg.reply.try_send(result)?;

        Ok(())
    }

    /// Remove a file on the remote filesystem.
    pub fn remove_file(&mut self, sess: &mut SessionWrap, msg: &RemoveFile) -> anyhow::Result<()> {
        let result = self.init_sftp(sess).and_then(|sftp| sftp.unlink(&msg.file));
        msg.reply.try_send(result)?;

        Ok(())
    }

    /// Initialize the sftp channel if not already created, returning a mutable reference to it
    fn init_sftp<'a>(&mut self, sess: &'a mut SessionWrap) -> SftpChannelResult<&'a mut SftpWrap> {
        match sess {
            SessionWrap::Ssh2(sess) => {
                if sess.sftp.is_none() {
                    sess.sftp = Some(SftpWrap::Ssh2(sess.sess.sftp()?));
                }
                Ok(sess.sftp.as_mut().expect("sftp should have been set above"))
            }
            SessionWrap::LibSsh(sess) => {
                if sess.sftp.is_none() {
                    sess.sftp = Some(SftpWrap::LibSsh(sess.sess.sftp()?));
                }
                Ok(sess.sftp.as_mut().expect("sftp should have been set above"))
            }
        }
    }
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
