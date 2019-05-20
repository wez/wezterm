use super::ownedhandle::OwnedHandle;
use super::WinChild;
use crate::cmdbuilder::CommandBuilder;
use crate::win::winpty::safe::{
    AgentFlags, MouseMode, SpawnConfig, SpawnFlags, Timeout, WinPty, WinPtyConfig,
};
use crate::{Child, MasterPty, PtySize, PtySystem, SlavePty};
use failure::{bail, Error};
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::Path;
use std::sync::{Arc, Mutex};

mod safe;
mod sys;

struct Inner {
    pty: WinPty,
    size: PtySize,
    reader: OwnedHandle,
    writer: OwnedHandle,
}

#[derive(Clone)]
pub struct WinPtyMasterPty {
    inner: Arc<Mutex<Inner>>,
}

pub struct WinPtySlavePty {
    inner: Arc<Mutex<Inner>>,
}

impl MasterPty for WinPtyMasterPty {
    fn resize(&self, size: PtySize) -> Result<(), Error> {
        let mut inner = self.inner.lock().unwrap();
        if inner.pty.set_size(size.cols as i32, size.rows as i32)? {
            inner.size = size;
            Ok(())
        } else {
            bail!("WinPtyMasterPty::resize returned false");
        }
    }

    fn get_size(&self) -> Result<PtySize, Error> {
        Ok(self.inner.lock().unwrap().size)
    }

    fn try_clone_reader(&self) -> Result<Box<std::io::Read + Send>, Error> {
        Ok(Box::new(self.inner.lock().unwrap().reader.try_clone()?))
    }
}

impl std::io::Write for WinPtyMasterPty {
    fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
        self.inner.lock().unwrap().writer.write(buf)
    }
    fn flush(&mut self) -> Result<(), std::io::Error> {
        Ok(())
    }
}

impl SlavePty for WinPtySlavePty {
    fn spawn_command(&self, cmd: CommandBuilder) -> Result<Box<Child>, Error> {
        let (exe, cmdline) = cmd.cmdline()?;
        let cmd_os = OsString::from_wide(&cmdline);
        eprintln!(
            "Running: module: {} {:?}",
            Path::new(&OsString::from_wide(&exe)).display(),
            cmd_os
        );

        let spawn_config = SpawnConfig::new(
            SpawnFlags::AUTO_SHUTDOWN | SpawnFlags::EXIT_AFTER_SHUTDOWN,
            Some(exe),
            Some(cmdline),
            None, // cwd
            None, // env
        )?;

        let mut inner = self.inner.lock().unwrap();
        let spawned = inner.pty.spawn(&spawn_config)?;

        let child = WinChild {
            proc: spawned.process_handle,
        };

        Ok(Box::new(child))
    }
}

pub struct WinPtySystem {}
impl PtySystem for WinPtySystem {
    fn openpty(&self, size: PtySize) -> Result<(Box<MasterPty>, Box<SlavePty>), Error> {
        let mut config = WinPtyConfig::new(AgentFlags::empty())?;

        config.set_initial_size(size.cols as i32, size.rows as i32);
        config.set_mouse_mode(MouseMode::Auto);
        config.set_agent_timeout(Timeout::Milliseconds(10_000));

        let pty = config.open()?;

        let reader = pty.conout()?;
        let writer = pty.conin()?;

        let inner = Arc::new(Mutex::new(Inner {
            pty,
            reader,
            writer,
            size,
        }));

        let master = WinPtyMasterPty {
            inner: Arc::clone(&inner),
        };
        let slave = WinPtySlavePty { inner };

        Ok((Box::new(master), Box::new(slave)))
    }
}
