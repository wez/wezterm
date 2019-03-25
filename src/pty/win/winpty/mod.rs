use super::cmdline::CommandBuilder;
use super::ownedhandle::OwnedHandle;
use super::{winsize, Child};
use failure::Error;
use safe::{AgentFlags, MouseMode, SpawnConfig, SpawnFlags, Timeout, WinPty, WinPtyConfig};
use std::ffi::{OsStr, OsString};
use std::os::windows::ffi::OsStringExt;
use std::path::Path;
use std::sync::{Arc, Mutex};

mod safe;
mod sys;

#[derive(Debug)]
pub struct Command {
    builder: CommandBuilder,
}

impl Command {
    pub fn new<S: AsRef<OsStr>>(program: S) -> Self {
        Self {
            builder: CommandBuilder::new(program),
        }
    }

    pub fn arg<S: AsRef<OsStr>>(&mut self, arg: S) -> &mut Command {
        self.builder.arg(arg);
        self
    }

    pub fn args<I, S>(&mut self, args: I) -> &mut Command
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.builder.args(args);
        self
    }

    pub fn env<K, V>(&mut self, key: K, val: V) -> &mut Command
    where
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        self.builder.env(key, val);
        self
    }
}

struct Inner {
    pty: WinPty,
    size: winsize,
    reader: OwnedHandle,
    writer: OwnedHandle,
}

#[derive(Clone)]
pub struct MasterPty {
    inner: Arc<Mutex<Inner>>,
}

pub struct SlavePty {
    inner: Arc<Mutex<Inner>>,
}

impl MasterPty {
    pub fn resize(
        &self,
        num_rows: u16,
        num_cols: u16,
        pixel_width: u16,
        pixel_height: u16,
    ) -> Result<(), Error> {
        let mut inner = self.inner.lock().unwrap();
        if inner.pty.set_size(num_cols as i32, num_rows as i32)? {
            inner.size = winsize {
                ws_row: num_rows,
                ws_col: num_cols,
                ws_xpixel: pixel_width,
                ws_ypixel: pixel_height,
            };
            Ok(())
        } else {
            bail!("MasterPty::resize returned false");
        }
    }

    pub fn get_size(&self) -> Result<winsize, Error> {
        Ok(self.inner.lock().unwrap().size)
    }

    pub fn try_clone_reader(&self) -> Result<Box<std::io::Read + Send>, Error> {
        Ok(Box::new(self.inner.lock().unwrap().reader.try_clone()?))
    }
}

impl std::io::Write for MasterPty {
    fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
        self.inner.lock().unwrap().writer.write(buf)
    }
    fn flush(&mut self) -> Result<(), std::io::Error> {
        Ok(())
    }
}

impl SlavePty {
    pub fn spawn_command(self, cmd: Command) -> Result<Child, Error> {
        let (exe, cmdline) = cmd.builder.cmdline()?;
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

        let child = Child {
            proc: spawned.process_handle,
        };

        Ok(child)
    }
}

pub fn openpty(
    num_rows: u16,
    num_cols: u16,
    pixel_width: u16,
    pixel_height: u16,
) -> Result<(MasterPty, SlavePty), Error> {
    let mut config = WinPtyConfig::new(AgentFlags::empty())?;

    config.set_initial_size(num_cols as i32, num_rows as i32);
    config.set_mouse_mode(MouseMode::Auto);
    config.set_agent_timeout(Timeout::Milliseconds(10_000));

    let pty = config.open()?;

    let reader = pty.conout()?;
    let writer = pty.conin()?;
    let size = winsize {
        ws_row: num_rows,
        ws_col: num_cols,
        ws_xpixel: pixel_width,
        ws_ypixel: pixel_height,
    };

    let inner = Arc::new(Mutex::new(Inner {
        pty,
        reader,
        writer,
        size,
    }));

    let master = MasterPty {
        inner: Arc::clone(&inner),
    };
    let slave = SlavePty { inner };

    Ok((master, slave))
}
