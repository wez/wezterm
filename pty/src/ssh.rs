//! This module implements a remote pty via ssh2.
//! While it offers a `PtySystem` implementation on the `SshSession`
//! struct, we don't include ssh in `PtySystemSelection` because
//! there is a non-trivial amount of setup that is required to
//! initiate a connection somewhere and to authenticate that session
//! before we can get to a point where `openpty` will be able to run.
use crate::{Child, CommandBuilder, ExitStatus, MasterPty, PtyPair, PtySize, PtySystem, SlavePty};
use filedescriptor::{AsRawSocketDescriptor, POLLIN};
use ssh2::{Channel, Session};
use std::collections::HashMap;
use std::io::Result as IoResult;
use std::io::{Read, Write};
use std::sync::{Arc, Condvar, Mutex};

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
    term: String,
    /// an instance of SshReader owns the wait for read and subsequent
    /// wakeup broadcast
    waiting_for_read: bool,
}

#[derive(Debug)]
struct SessionHolder {
    locked_inner: Mutex<SessionInner>,
    read_waiters: Condvar,
}

// An anemic impl of Debug to satisfy some indirect trait bounds
impl std::fmt::Debug for SessionInner {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        fmt.debug_struct("SessionInner")
            .field("next_channel_id", &self.next_channel_id)
            .finish()
    }
}

/// The `SshSession` struct wraps an `ssh2::Session` instance.
/// The session is expected to have been pre-connected and pre-authenticated
/// by the calling the application.
/// Once established and wrapped into an `SshSession`, the `SshSession`
/// implements the `PtySystem` trait and exposes the `openpty` function
/// that can be used to return a remote pty via ssh.
pub struct SshSession {
    inner: Arc<SessionHolder>,
}

impl SshSession {
    /// Wrap an `ssh2::Session` in such a way that we can safely map it
    /// into the `portable-pty` object model.
    /// The `ssh2::Session` must be pre-connected (eg: `ssh2::Session::handshake`
    /// must have been successfully completed) and pre-authenticated so that
    /// internal calls made to `ssh2::Channel::exec` can be made.
    /// The `term` parameter specifies the term name for the remote host in
    /// the case that a pty needs to be allocated.
    pub fn new(session: Session, term: &str) -> Self {
        Self {
            inner: Arc::new(SessionHolder {
                locked_inner: Mutex::new(SessionInner {
                    session,
                    ptys: HashMap::new(),
                    next_channel_id: 1,
                    term: term.to_string(),
                    waiting_for_read: false,
                }),
                read_waiters: Condvar::new(),
            }),
        }
    }
}

impl PtySystem for SshSession {
    fn openpty(&self, size: PtySize) -> anyhow::Result<PtyPair> {
        let mut inner = self.inner.locked_inner.lock().unwrap();
        let mut channel = inner.session.channel_session()?;
        channel.handle_extended_data(ssh2::ExtendedData::Merge)?;
        channel.request_pty(
            &inner.term,
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
    inner: Arc<SessionHolder>,
}

impl PtyHandle {
    /// Acquire the session mutex and then perform a lambda on the Channel
    fn with_channel<R, F: FnMut(&mut Channel) -> R>(&self, mut f: F) -> R {
        let mut inner = self.inner.locked_inner.lock().unwrap();
        f(&mut inner.ptys.get_mut(&self.id).unwrap().channel)
    }

    /// Acquire the session mutex and then perform a lambda on the SshPty
    fn with_pty<R, F: FnMut(&mut SshPty) -> R>(&self, mut f: F) -> R {
        let mut inner = self.inner.locked_inner.lock().unwrap();
        f(&mut inner.ptys.get_mut(&self.id).unwrap())
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
    fn resize(&self, size: PtySize) -> anyhow::Result<()> {
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

    fn get_size(&self) -> anyhow::Result<PtySize> {
        Ok(self.pty.with_pty(|pty| pty.size))
    }

    fn try_clone_reader(&self) -> anyhow::Result<Box<dyn std::io::Read + Send>> {
        Ok(Box::new(SshReader {
            pty: self.pty.clone(),
        }))
    }

    fn try_clone_writer(&self) -> anyhow::Result<Box<dyn std::io::Write + Send>> {
        Ok(Box::new(SshMaster {
            pty: self.pty.clone(),
        }))
    }

    #[cfg(unix)]
    fn process_group_leader(&self) -> Option<libc::pid_t> {
        // N/A: there is no local process
        None
    }
}

struct SshSlave {
    pty: PtyHandle,
}

impl SlavePty for SshSlave {
    fn spawn_command(&self, cmd: CommandBuilder) -> anyhow::Result<Box<dyn Child + Send + Sync>> {
        self.pty.with_channel(|channel| {
            for (key, val) in cmd.iter_env_as_str() {
                if let Err(err) = channel.setenv(key, val) {
                    // Depending on the server configuration, a given
                    // setenv request may not succeed, but that doesn't
                    // prevent the connection from being set up.
                    log::error!("ssh: setenv {}={} failed: {}", key, val, err);
                }
            }

            if cmd.is_default_prog() {
                channel.shell()?;
            } else {
                let command = cmd.as_unix_command_line()?;
                channel.exec(&command)?;
            }

            let child: Box<dyn Child + Send + Sync> = Box::new(SshChild {
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
        let mut lock = self.pty.inner.locked_inner.try_lock();
        if let Ok(ref mut inner) = lock {
            let ssh_pty = inner.ptys.get_mut(&self.pty.id).unwrap();
            if ssh_pty.channel.eof() {
                Ok(Some(ExitStatus::with_exit_code(
                    ssh_pty.channel.exit_status()? as u32,
                )))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
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

    fn process_id(&self) -> Option<u32> {
        None
    }

    #[cfg(windows)]
    fn as_raw_handle(&self) -> Option<std::os::windows::io::RawHandle> {
        None
    }
}

struct SshReader {
    pty: PtyHandle,
}

impl Read for SshReader {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        loop {
            let mut inner = self.pty.inner.locked_inner.lock().unwrap();

            inner.session.set_blocking(false);
            let res = inner.ptys.get_mut(&self.pty.id).unwrap().channel.read(buf);
            inner.session.set_blocking(true);
            match res {
                Ok(size) => return Ok(size),
                Err(err) => match err.kind() {
                    std::io::ErrorKind::WouldBlock => {}
                    _ => return Err(err),
                },
            };

            // No data available for this channel, so we'll wait.
            // If we're the first SshReader to do this, we'll perform the
            // OS level poll() call for ourselves, otherwise we'll block
            // on the condvar
            if inner.waiting_for_read {
                self.pty.inner.read_waiters.wait(inner).ok();
            } else {
                let socket = inner.session.as_socket_descriptor();

                // We own waiting for read
                inner.waiting_for_read = true;

                // Unlock and wait
                drop(inner);

                let mut pfd = [filedescriptor::pollfd {
                    fd: socket,
                    events: POLLIN,
                    revents: 0,
                }];
                filedescriptor::poll(&mut pfd, None).ok();

                // re-acquire the lock to release our ownership of the poll
                // and to wake up the others
                let mut inner = self.pty.inner.locked_inner.lock().unwrap();
                inner.waiting_for_read = false;

                // Wake all readers and we'll all race to read our next
                // iteration
                self.pty.inner.read_waiters.notify_all();
                drop(inner);
            }
        }
    }
}
