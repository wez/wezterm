use crate::pty::{NewPty, ResizePty};
use libssh_rs as libssh;
use portable_pty::ExitStatus;

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
