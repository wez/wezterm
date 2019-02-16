use failure::Error;
use std::io;
use std::process::{Child, Command};

pub struct MasterPty {}
pub struct SlavePty {}
pub struct winsize {
    pub ws_row: u16,
    pub ws_col: u16,
    pub ws_xpixel: u16,
    pub ws_ypixel: u16,
}

impl MasterPty {
    pub fn resize(
        &self,
        num_rows: u16,
        num_cols: u16,
        pixel_width: u16,
        pixel_height: u16,
    ) -> Result<(), Error> {
        bail!("MasterPty::resize not implemented")
    }

    pub fn get_size(&self) -> Result<winsize, Error> {
        bail!("MasterPty::get_size not implemented")
    }

    pub fn try_clone(&self) -> Result<Self, Error> {
        bail!("MasterPty::try_clone not implemented")
    }

    pub fn clear_nonblocking(&self) -> Result<(), Error> {
        unimplemented!();
    }
}

impl io::Write for MasterPty {
    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        unimplemented!();
    }
    fn flush(&mut self) -> Result<(), io::Error> {
        Ok(())
    }
}

impl io::Read for MasterPty {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, io::Error> {
        unimplemented!();
    }
}

impl SlavePty {
    pub fn spawn_command(self, mut cmd: Command) -> Result<Child, Error> {
        bail!("spawn_command not implemented")
    }
}

pub fn openpty(
    num_rows: u16,
    num_cols: u16,
    pixel_width: u16,
    pixel_height: u16,
) -> Result<(MasterPty, SlavePty), Error> {
    bail!("openpty not implemented")
}
