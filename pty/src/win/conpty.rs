use crate::cmdbuilder::CommandBuilder;
use crate::win::psuedocon::PsuedoCon;
use crate::{Child, MasterPty, PtyPair, PtySize, PtySystem, SlavePty};
use anyhow::Error;
use filedescriptor::{FileDescriptor, Pipe};
use std::sync::{Arc, Mutex};
use winapi::um::wincon::COORD;

#[derive(Default)]
pub struct ConPtySystem {}

impl PtySystem for ConPtySystem {
    fn openpty(&self, size: PtySize) -> anyhow::Result<PtyPair> {
        let stdin = Pipe::new()?;
        let stdout = Pipe::new()?;

        let con = PsuedoCon::new(
            COORD {
                X: size.cols as i16,
                Y: size.rows as i16,
            },
            stdin.read,
            stdout.write,
        )?;

        let master = ConPtyMasterPty {
            inner: Arc::new(Mutex::new(Inner {
                con,
                readable: stdout.read,
                writable: Some(stdin.write),
                size,
            })),
        };

        let slave = ConPtySlavePty {
            inner: master.inner.clone(),
        };

        Ok(PtyPair {
            master: Box::new(master),
            slave: Box::new(slave),
        })
    }
}

struct Inner {
    con: PsuedoCon,
    readable: FileDescriptor,
    writable: Option<FileDescriptor>,
    size: PtySize,
}

impl Inner {
    pub fn resize(
        &mut self,
        num_rows: u16,
        num_cols: u16,
        pixel_width: u16,
        pixel_height: u16,
    ) -> Result<(), Error> {
        self.con.resize(COORD {
            X: num_cols as i16,
            Y: num_rows as i16,
        })?;
        self.size = PtySize {
            rows: num_rows,
            cols: num_cols,
            pixel_width,
            pixel_height,
        };
        Ok(())
    }
}

#[derive(Clone)]
pub struct ConPtyMasterPty {
    inner: Arc<Mutex<Inner>>,
}

pub struct ConPtySlavePty {
    inner: Arc<Mutex<Inner>>,
}

impl MasterPty for ConPtyMasterPty {
    fn resize(&self, size: PtySize) -> anyhow::Result<()> {
        let mut inner = self.inner.lock().unwrap();
        inner.resize(size.rows, size.cols, size.pixel_width, size.pixel_height)
    }

    fn get_size(&self) -> Result<PtySize, Error> {
        let inner = self.inner.lock().unwrap();
        Ok(inner.size.clone())
    }

    fn try_clone_reader(&self) -> anyhow::Result<Box<dyn std::io::Read + Send>> {
        Ok(Box::new(self.inner.lock().unwrap().readable.try_clone()?))
    }

    fn take_writer(&self) -> anyhow::Result<Box<dyn std::io::Write + Send>> {
        Ok(Box::new(
            self.inner
                .lock()
                .unwrap()
                .writable
                .take()
                .ok_or_else(|| anyhow::anyhow!("writer already taken"))?,
        ))
    }
}

impl SlavePty for ConPtySlavePty {
    fn spawn_command(&self, cmd: CommandBuilder) -> anyhow::Result<Box<dyn Child + Send + Sync>> {
        let inner = self.inner.lock().unwrap();
        let child = inner.con.spawn_command(cmd)?;
        Ok(Box::new(child))
    }
}
