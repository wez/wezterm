//! This module implements a remote pty via ssh2.
//! While it offers a `PtySystem` implementation on the `SshSession`
//! struct, we don't include ssh in `PtySystemSelection` because
//! there is a non-trivial amount of setup that is required to
//! initiate a connection somewhere and to authenticate that session
//! before we can get to a point where `openpty` will be able to run.
use crate::{Child, CommandBuilder, ExitStatus, MasterPty, PtyPair, PtySize, PtySystem, SlavePty};
use failure::{format_err, Fallible};
use filedescriptor::AsRawSocketDescriptor;
use ssh2::{Channel, Session};
use std::collections::HashMap;
use std::io::Result as IoResult;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

/// Represents a pty channel within a session.
struct SshPty {
    channel: Channel,
    /// The size that we last set; we need to remember it in order to
    /// return it via `get_size`.
    size: PtySize,
}

/// The internal state that tracks the ssh session.
/// It owns the Session and indirectly owns the Channel instances.
/// The ownership is important: both must be protected by the same
/// mutable borrow in order to respect the threading model of libssh2.
/// We do so by ensuring that a Mutex wraps SessionInner.
struct SessionInner {
    session: Session,
    ptys: HashMap<usize, SshPty>,
    next_channel_id: usize,
}

// An anemic impl of Debug to satisfy some indirect trait bounds
impl std::fmt::Debug for SessionInner {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        fmt.debug_struct("SessionInner")
            .field("next_channel_id", &self.next_channel_id)
            .finish()
    }
}

// This is actually safe because the unsafety is caused by the raw channel
// pointers embedded in SshPty::channel.  Those are only unsafe when used
// with multiple threads.  It is safe here because we ensure that all
// accesses are made via SessionInner which is protected via a Mutex
unsafe impl Send for SessionInner {}

/// The `SshSession` struct wraps an `ssh2::Session` instance.
/// The session is expected to have been pre-connected and pre-authenticated
/// by the calling the application.
/// Once established and wrapped into an `SshSession`, the `SshSession`
/// implements the `PtySystem` trait and exposes the `openpty` function
/// that can be used to return a remote pty via ssh.
pub struct SshSession {
    inner: Arc<Mutex<SessionInner>>,
}

impl SshSession {
    /// Wrap an `ssh2::Session` in such a way that we can safely map it
    /// into the `portable-pty` object model.
    /// The `ssh2::Session` must be pre-connected (eg: `ssh2::Session::handshake`
    /// must have been successfully completed) and pre-authenticated so that
    /// internal calls made to `ssh2::Channel::exec` can be made.
    pub fn new(session: Session) -> Self {
        Self {
            inner: Arc::new(Mutex::new(SessionInner {
                session,
                ptys: HashMap::new(),
                next_channel_id: 1,
            })),
        }
    }
}

impl PtySystem for SshSession {
    fn openpty(&self, size: PtySize) -> Fallible<PtyPair> {
        let mut inner = self.inner.lock().unwrap();
        let mut channel = inner.session.channel_session()?;
        channel.handle_extended_data(ssh2::ExtendedData::Merge)?;
        channel.request_pty(
            // Unfortunately we need to pass in *something* for the
            // terminal name here to satisfy the ssh spec.
            // We don't know what the TERM environment might be
            // until we get to `SlavePty::spawn_command`.
            // We use xterm here because it is pretty ubiquitous.
            "xterm",
            None,
            Some((
                size.cols.into(),
                size.rows.into(),
                size.pixel_width.into(),
                size.pixel_height.into(),
            )),
        )?;

        let id = {
            let id = inner.next_channel_id;
            inner.next_channel_id += 1;
            inner.ptys.insert(id, SshPty { channel, size });
            id
        };
        let pty = PtyHandle {
            id,
            inner: Arc::clone(&self.inner),
        };

        Ok(PtyPair {
            slave: Box::new(SshSlave { pty: pty.clone() }),
            master: Box::new(SshMaster { pty }),
        })
    }
}

/// Represents a handle to a Channel
#[derive(Clone, Debug)]
struct PtyHandle {
    id: usize,
    inner: Arc<Mutex<SessionInner>>,
}

impl PtyHandle {
    /// Acquire the session mutex and then perform a lambda on the Channel
    fn with_channel<R, F: FnMut(&mut Channel) -> R>(&self, mut f: F) -> R {
        let mut inner = self.inner.lock().unwrap();
        f(&mut inner.ptys.get_mut(&self.id).unwrap().channel)
    }

    /// Acquire the session mutex and then perform a lambda on the SshPty
    fn with_pty<R, F: FnMut(&mut SshPty) -> R>(&self, mut f: F) -> R {
        let mut inner = self.inner.lock().unwrap();
        f(&mut inner.ptys.get_mut(&self.id).unwrap())
    }

    fn as_socket_descriptor(&self) -> filedescriptor::SocketDescriptor {
        let inner = self.inner.lock().unwrap();
        let stream = inner.session.tcp_stream();
        stream.as_ref().unwrap().as_socket_descriptor()
    }
}

struct SshMaster {
    pty: PtyHandle,
}

impl Write for SshMaster {
    fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
        self.pty.with_channel(|channel| channel.write(buf))
    }

    fn flush(&mut self) -> Result<(), std::io::Error> {
        self.pty.with_channel(|channel| channel.flush())
    }
}

impl MasterPty for SshMaster {
    fn resize(&self, size: PtySize) -> Fallible<()> {
        self.pty.with_pty(|pty| {
            pty.channel.request_pty_size(
                size.cols.into(),
                size.rows.into(),
                Some(size.pixel_width.into()),
                Some(size.pixel_height.into()),
            )?;
            pty.size = size;
            Ok(())
        })
    }

    fn get_size(&self) -> Fallible<PtySize> {
        Ok(self.pty.with_pty(|pty| pty.size))
    }

    fn try_clone_reader(&self) -> Fallible<Box<dyn std::io::Read + Send>> {
        Ok(Box::new(SshReader {
            pty: self.pty.clone(),
        }))
    }
}

struct SshSlave {
    pty: PtyHandle,
}

impl SlavePty for SshSlave {
    fn spawn_command(&self, cmd: CommandBuilder) -> Fallible<Box<dyn Child>> {
        self.pty.with_channel(|channel| {
            for (key, val) in cmd.iter_env_as_str() {
                channel
                    .setenv(key, val)
                    .map_err(|e| format_err!("ssh: setenv {}={} failed: {}", key, val, e))?;
            }

            let command = cmd.as_unix_command_line()?;
            channel.exec(&command)?;

            let child: Box<dyn Child> = Box::new(SshChild {
                pty: self.pty.clone(),
            });

            Ok(child)
        })
    }
}

#[derive(Debug)]
struct SshChild {
    pty: PtyHandle,
}

impl Child for SshChild {
    fn try_wait(&mut self) -> IoResult<Option<ExitStatus>> {
        self.pty.with_channel(|channel| {
            if channel.eof() {
                Ok(Some(ExitStatus::with_exit_code(
                    channel.exit_status()? as u32
                )))
            } else {
                Ok(None)
            }
        })
    }

    fn kill(&mut self) -> IoResult<()> {
        self.pty.with_channel(|channel| channel.send_eof())?;
        Ok(())
    }

    fn wait(&mut self) -> IoResult<ExitStatus> {
        self.pty.with_channel(|channel| {
            channel.close()?;
            channel.wait_close()?;
            Ok(ExitStatus::with_exit_code(channel.exit_status()? as u32))
        })
    }
}

struct SshReader {
    pty: PtyHandle,
}

impl Read for SshReader {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        // A blocking read, but we don't want to own the mutex while we
        // sleep, so we manually poll the underlying socket descriptor
        // and then use a non-blocking read to read the actual data
        let socket = self.pty.as_socket_descriptor();
        loop {
            // Wait for input on the descriptor
            let mut pfd = [filedescriptor::pollfd {
                fd: socket,
                events: filedescriptor::POLLIN,
                revents: 0,
            }];
            filedescriptor::poll(&mut pfd, None).ok();

            // a read won't block, so ask libssh2 for data from the
            // associated channel, but do not block!
            let res = {
                let mut inner = self.pty.inner.lock().unwrap();
                inner.session.set_blocking(false);
                let res = inner.ptys.get_mut(&self.pty.id).unwrap().channel.read(buf);
                inner.session.set_blocking(true);
                res
            };

            // If we have data or an error, return it, otherwise let's
            // try again!
            match res {
                Ok(len) => return Ok(len),
                Err(err) => match err.kind() {
                    std::io::ErrorKind::WouldBlock => continue,
                    _ => return Err(err),
                },
            }
        }
    }
}
