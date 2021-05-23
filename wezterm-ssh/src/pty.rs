use crate::session::{ChannelId, ChannelInfo, DescriptorState, SessionRequest, SessionSender};
use filedescriptor::{socketpair, FileDescriptor};
use portable_pty::{ExitStatus, PtySize};
use smol::channel::{bounded, Receiver, Sender, TryRecvError};
use std::collections::HashMap;
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::sync::Mutex;

#[derive(Debug)]
pub(crate) struct NewPty {
    pub term: String,
    pub size: PtySize,
    pub command_line: Option<String>,
    pub env: Option<HashMap<String, String>>,
    pub reply: Sender<(SshPty, SshChildProcess)>,
}

#[derive(Debug)]
pub(crate) struct ResizePty {
    pub channel: ChannelId,
    pub size: PtySize,
    pub reply: Sender<()>,
}

#[derive(Debug)]
pub struct SshPty {
    pub(crate) channel: ChannelId,
    pub(crate) tx: Option<SessionSender>,
    pub(crate) reader: FileDescriptor,
    pub(crate) writer: FileDescriptor,
    pub(crate) size: Mutex<PtySize>,
}

impl std::io::Write for SshPty {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.writer.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}

impl portable_pty::MasterPty for SshPty {
    fn resize(&self, size: PtySize) -> anyhow::Result<()> {
        let (tx, rx) = bounded(1);
        self.tx
            .as_ref()
            .unwrap()
            .try_send(SessionRequest::ResizePty(ResizePty {
                channel: self.channel,
                size,
                reply: tx,
            }))?;

        smol::block_on(rx.recv())?;
        *self.size.lock().unwrap() = size;
        Ok(())
    }

    fn get_size(&self) -> anyhow::Result<PtySize> {
        Ok(*self.size.lock().unwrap())
    }

    fn try_clone_reader(&self) -> anyhow::Result<Box<(dyn Read + Send + 'static)>> {
        let reader = self.reader.try_clone()?;
        Ok(Box::new(reader))
    }

    fn try_clone_writer(&self) -> anyhow::Result<Box<(dyn Write + Send + 'static)>> {
        let writer = self.writer.try_clone()?;
        Ok(Box::new(writer))
    }

    #[cfg(unix)]
    fn process_group_leader(&self) -> Option<i32> {
        // It's not local, so there's no meaningful leader
        None
    }
}

#[derive(Debug)]
pub struct SshChildProcess {
    pub(crate) channel: ChannelId,
    pub(crate) tx: Option<SessionSender>,
    pub(crate) exit: Receiver<ExitStatus>,
    pub(crate) exited: Option<ExitStatus>,
}

impl portable_pty::Child for SshChildProcess {
    fn try_wait(&mut self) -> std::io::Result<Option<ExitStatus>> {
        if let Some(status) = self.exited.as_ref() {
            return Ok(Some(status.clone()));
        }
        match self.exit.try_recv() {
            Ok(status) => {
                self.exited.replace(status.clone());
                Ok(Some(status))
            }
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Closed) => {
                let status = ExitStatus::with_exit_code(1);
                self.exited.replace(status.clone());
                Ok(Some(status))
            }
        }
    }

    fn kill(&mut self) -> std::io::Result<()> {
        // There is no way to send a signal via libssh2.
        // Just pretend that we did. :-/
        Ok(())
    }

    fn wait(&mut self) -> std::io::Result<portable_pty::ExitStatus> {
        if let Some(status) = self.exited.as_ref() {
            return Ok(status.clone());
        }
        match smol::block_on(self.exit.recv()) {
            Ok(status) => {
                self.exited.replace(status.clone());
                Ok(status)
            }
            Err(_) => {
                let status = ExitStatus::with_exit_code(1);
                self.exited.replace(status.clone());
                Ok(status)
            }
        }
    }

    fn process_id(&self) -> Option<u32> {
        None
    }
}

impl crate::session::SessionInner {
    pub fn new_pty(&mut self, sess: &ssh2::Session, newpty: &NewPty) -> anyhow::Result<()> {
        sess.set_blocking(true);

        let mut channel = sess.channel_session()?;

        channel.handle_extended_data(ssh2::ExtendedData::Merge)?;

        /* libssh2 doesn't properly support agent forwarding
         * at this time:
         * <https://github.com/libssh2/libssh2/issues/535>
        if let Some("yes") = self.config.get("forwardagent").map(|s| s.as_str()) {
            log::info!("requesting agent forwarding");
            if let Err(err) = channel.request_auth_agent_forwarding() {
                log::error!("Failed to establish agent forwarding: {:#}", err);
            }
            log::info!("agent forwarding OK!");
        }
        */

        channel.request_pty(
            &newpty.term,
            None,
            Some((
                newpty.size.cols.into(),
                newpty.size.rows.into(),
                newpty.size.pixel_width.into(),
                newpty.size.pixel_height.into(),
            )),
        )?;

        if let Some(env) = &newpty.env {
            for (key, val) in env {
                if let Err(err) = channel.setenv(key, val) {
                    // Depending on the server configuration, a given
                    // setenv request may not succeed, but that doesn't
                    // prevent the connection from being set up.
                    log::warn!("ssh: setenv {}={} failed: {}", key, val, err);
                }
            }
        }

        if let Some(cmd) = &newpty.command_line {
            channel.exec(cmd)?;
        } else {
            channel.shell()?;
        }

        let channel_id = self.next_channel_id;
        self.next_channel_id += 1;

        let (write_to_stdin, mut read_from_stdin) = socketpair()?;
        let (mut write_to_stdout, read_from_stdout) = socketpair()?;

        read_from_stdin.set_non_blocking(true)?;
        write_to_stdout.set_non_blocking(true)?;

        let ssh_pty = SshPty {
            channel: channel_id,
            tx: None,
            reader: read_from_stdout,
            writer: write_to_stdin,
            size: Mutex::new(newpty.size),
        };

        let (exit_tx, exit_rx) = bounded(1);

        let child = SshChildProcess {
            channel: channel_id,
            tx: None,
            exit: exit_rx,
            exited: None,
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
                    fd: None,
                    buf: VecDeque::new(),
                },
            ],
        };

        newpty.reply.try_send((ssh_pty, child))?;
        self.channels.insert(channel_id, info);

        Ok(())
    }

    pub fn resize_pty(&mut self, sess: &ssh2::Session, resize: &ResizePty) -> anyhow::Result<()> {
        sess.set_blocking(true);

        let info = self
            .channels
            .get_mut(&resize.channel)
            .ok_or_else(|| anyhow::anyhow!("invalid channel id {}", resize.channel))?;
        info.channel.request_pty_size(
            resize.size.cols.into(),
            resize.size.rows.into(),
            Some(resize.size.pixel_width.into()),
            Some(resize.size.pixel_height.into()),
        )?;
        resize.reply.try_send(())?;
        Ok(())
    }
}
