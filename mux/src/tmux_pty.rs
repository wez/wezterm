use crate::{
    tmux::{RefTmuxRemotePane, TmuxCmdQueue, TmuxDomainState},
    tmux_commands::SendKeys,
};
use portable_pty::{Child, ExitStatus, MasterPty};
use std::{
    io::{Read, Write},
    sync::{Arc, Condvar, Mutex},
};

pub(crate) struct TmuxReader {
    rx: flume::Receiver<String>,
}

impl Read for TmuxReader {
    fn read(&mut self, mut buf: &mut [u8]) -> std::io::Result<usize> {
        match self.rx.recv() {
            Ok(str) => {
                return buf.write(str.as_bytes());
            }
            Err(_) => {
                return Ok(0);
            }
        }
    }
}

/// A local tmux pane(tab) based on a tmux pty
#[derive(Debug, Clone)]
pub(crate) struct TmuxPty {
    pub domain_id: usize,
    pub master_pane: RefTmuxRemotePane,
    pub rx: flume::Receiver<String>,
    pub cmd_queue: Arc<Mutex<TmuxCmdQueue>>,

    /// would be released by TmuxDomain when detatched
    pub active_lock: Arc<(Mutex<bool>, Condvar)>,
}

impl Write for TmuxPty {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let pane_id = {
            let pane_lock = self.master_pane.lock().unwrap();
            pane_lock.pane_id
        };
        log::trace!("pane:{}, content:{:?}", &pane_id, buf);
        let mut cmd_queue = self.cmd_queue.lock().unwrap();
        cmd_queue.push_back(Box::new(SendKeys {
            pane: pane_id,
            keys: buf.to_vec(),
        }));
        TmuxDomainState::schedule_send_next_command(self.domain_id);
        Ok(0)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Child for TmuxPty {
    fn try_wait(&mut self) -> std::io::Result<Option<portable_pty::ExitStatus>> {
        todo!()
    }

    fn kill(&mut self) -> std::io::Result<()> {
        todo!()
    }

    fn wait(&mut self) -> std::io::Result<portable_pty::ExitStatus> {
        let (lock, var) = &*self.active_lock;
        let mut released = lock.lock().unwrap();
        while !*released {
            released = var.wait(released).unwrap();
        }
        return Ok(ExitStatus::with_exit_code(0));
    }

    fn process_id(&self) -> Option<u32> {
        Some(0)
    }

    #[cfg(windows)]
    fn as_raw_handle(&self) -> Option<std::os::windows::io::RawHandle> {
        None
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
            domain_id: self.domain_id,
            master_pane: self.master_pane.clone(),
            rx: self.rx.clone(),
            cmd_queue: self.cmd_queue.clone(),
            active_lock: self.active_lock.clone(),
        }))
    }

    #[cfg(unix)]
    fn process_group_leader(&self) -> Option<libc::pid_t> {
        return None;
    }
}
