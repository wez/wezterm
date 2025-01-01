use crate::pty::{NewPty, ResizePty};
use portable_pty::ExitStatus;

pub(crate) enum ChannelWrap {
    #[cfg(feature = "ssh2")]
    Ssh2(ssh2::Channel),

    #[cfg(feature = "libssh-rs")]
    LibSsh(libssh_rs::Channel),
}

#[cfg(feature = "ssh2")]
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
            #[cfg(feature = "ssh2")]
            Self::Ssh2(chan) => {
                if chan.eof() && chan.wait_close().is_ok() {
                    if let Some(sig) = has_signal(chan) {
                        Some(ExitStatus::with_signal(
                            sig.exit_signal.as_deref().unwrap_or("Unknown signal"),
                        ))
                    } else if let Ok(status) = chan.exit_status() {
                        Some(ExitStatus::with_exit_code(status as _))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }

            #[cfg(feature = "libssh-rs")]
            Self::LibSsh(chan) => {
                if chan.is_eof() {
                    if let Some(status) = chan.get_exit_status() {
                        return Some(ExitStatus::with_exit_code(status as u32));
                    } else if let Some(status) = chan.get_exit_signal() {
                        return Some(ExitStatus::with_signal(
                            status.signal_name.as_deref().unwrap_or("unknown signal"),
                        ));
                    }
                }
                None
            }
        }
    }

    pub fn reader(&mut self, idx: usize) -> Box<dyn std::io::Read + '_> {
        match self {
            #[cfg(feature = "ssh2")]
            Self::Ssh2(chan) => Box::new(chan.stream(idx as i32)),

            #[cfg(feature = "libssh-rs")]
            Self::LibSsh(chan) => match idx {
                0 => Box::new(chan.stdout()),
                1 => Box::new(chan.stderr()),
                _ => panic!("wanted reader for idx={}", idx),
            },
        }
    }

    pub fn writer(&mut self) -> Box<dyn std::io::Write + '_> {
        match self {
            #[cfg(feature = "ssh2")]
            Self::Ssh2(chan) => Box::new(chan),

            #[cfg(feature = "libssh-rs")]
            Self::LibSsh(chan) => Box::new(chan.stdin()),
        }
    }

    pub fn close(&mut self) {
        match self {
            #[cfg(feature = "ssh2")]
            Self::Ssh2(chan) => {
                let _ = chan.close();
            }

            #[cfg(feature = "libssh-rs")]
            Self::LibSsh(chan) => {
                let _ = chan.close();
            }
        }
    }

    pub fn request_pty(&mut self, newpty: &NewPty) -> anyhow::Result<()> {
        match self {
            #[cfg(feature = "ssh2")]
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

            #[cfg(feature = "libssh-rs")]
            Self::LibSsh(chan) => Ok(chan.request_pty(
                &newpty.term,
                newpty.size.cols.into(),
                newpty.size.rows.into(),
            )?),
        }
    }

    pub fn request_env(&mut self, name: &str, value: &str) -> anyhow::Result<()> {
        match self {
            #[cfg(feature = "ssh2")]
            Self::Ssh2(chan) => Ok(chan.setenv(name, value)?),

            #[cfg(feature = "libssh-rs")]
            Self::LibSsh(chan) => Ok(chan.request_env(name, value)?),
        }
    }

    pub fn request_exec(&mut self, command_line: &str) -> anyhow::Result<()> {
        match self {
            #[cfg(feature = "ssh2")]
            Self::Ssh2(chan) => Ok(chan.exec(command_line)?),

            #[cfg(feature = "libssh-rs")]
            Self::LibSsh(chan) => Ok(chan.request_exec(command_line)?),
        }
    }

    pub fn request_shell(&mut self) -> anyhow::Result<()> {
        match self {
            #[cfg(feature = "ssh2")]
            Self::Ssh2(chan) => Ok(chan.shell()?),

            #[cfg(feature = "libssh-rs")]
            Self::LibSsh(chan) => Ok(chan.request_shell()?),
        }
    }

    pub fn request_auth_agent_forwarding(&mut self) -> anyhow::Result<()> {
        match self {
            /* libssh2 doesn't properly support agent forwarding
             * at this time:
             * <https://github.com/libssh2/libssh2/issues/535> */
            #[cfg(feature = "ssh2")]
            Self::Ssh2(_chan) => Err(anyhow::anyhow!(
                "ssh2 does not support request_auth_agent_forwarding"
            )),

            #[cfg(feature = "libssh-rs")]
            Self::LibSsh(chan) => Ok(chan.request_auth_agent()?),
        }
    }

    pub fn resize_pty(&mut self, resize: &ResizePty) -> anyhow::Result<()> {
        match self {
            #[cfg(feature = "ssh2")]
            Self::Ssh2(chan) => Ok(chan.request_pty_size(
                resize.size.cols.into(),
                resize.size.rows.into(),
                Some(resize.size.pixel_width.into()),
                Some(resize.size.pixel_height.into()),
            )?),

            #[cfg(feature = "libssh-rs")]
            Self::LibSsh(chan) => {
                Ok(chan.change_pty_size(resize.size.cols.into(), resize.size.rows.into())?)
            }
        }
    }

    pub fn send_signal(
        &mut self,
        #[cfg_attr(not(feature = "libssh-rs"), allow(unused_variables))] signame: &str,
    ) -> anyhow::Result<()> {
        match self {
            #[cfg(feature = "ssh2")]
            Self::Ssh2(_) => Ok(()),

            #[cfg(feature = "libssh-rs")]
            Self::LibSsh(chan) => Ok(chan.request_send_signal(signame)?),
        }
    }
}
