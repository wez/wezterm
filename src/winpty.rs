use failure::Error;
use std::process::{Child, Command};

pub struct MasterPty {}
pub struct SlavePty {}
pub struct winsize {}

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
