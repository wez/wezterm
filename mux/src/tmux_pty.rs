use flume;
use portable_pty::MasterPty;
use std::io::{Read, Write};

use crate::tmux::RefTmuxRemotePane;

pub(crate) struct TmuxReader {
    rx: flume::Receiver<String>,
}

impl Read for TmuxReader {
    fn read(&mut self, mut buf: &mut [u8]) -> std::io::Result<usize> {
        match self.rx.try_recv() {
            Ok(str) => {
                return buf.write(str.as_bytes());
            }
            Err(_) => {
                return Ok(0);
            }
        }
    }
}

// A local tmux pane(tab) based on a tmux pty
pub(crate) struct TmuxPty {
    master_pane: RefTmuxRemotePane,
    rx: flume::Receiver<String>,
    // TODO: wx
}

impl Write for TmuxPty {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        // TODO: write to wx of pty
        Ok(0)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl MasterPty for TmuxPty {
    fn resize(&self, size: portable_pty::PtySize) -> Result<(), anyhow::Error> {
        // TODO: perform pane resize
        Ok(())
    }

    fn get_size(&self) -> Result<portable_pty::PtySize, anyhow::Error> {
        let pane = self.master_pane.lock().unwrap();
        Ok(portable_pty::PtySize {
            rows: pane.pane_height as u16,
            cols: pane.pane_width as u16,
            pixel_width: 0,
            pixel_height: 0,
        })
    }

    fn try_clone_reader(&self) -> Result<Box<dyn std::io::Read + Send>, anyhow::Error> {
        Ok(Box::new(TmuxReader {
            rx: self.rx.clone(),
        }))
    }

    fn try_clone_writer(&self) -> Result<Box<dyn std::io::Write + Send>, anyhow::Error> {
        Ok(Box::new(TmuxPty {
            master_pane: self.master_pane.clone(),
            rx: self.rx.clone(),
        }))
    }

    fn process_group_leader(&self) -> Option<libc::pid_t> {
        return None;
    }
}
