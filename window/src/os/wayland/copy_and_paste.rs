use anyhow::{anyhow, bail};
use smithay_client_toolkit as toolkit;
use std::io::Write;
use std::os::fd::AsRawFd;
use std::sync::{Arc, Mutex};
use toolkit::data_device_manager::data_offer::SelectionOffer;
use toolkit::data_device_manager::{ReadPipe, WritePipe};
use toolkit::primary_selection::device::PrimarySelectionDeviceHandler;
use toolkit::primary_selection::selection::PrimarySelectionSourceHandler;
use wayland_protocols::wp::primary_selection::zv1::client::zwp_primary_selection_device_v1::ZwpPrimarySelectionDeviceV1;
use wayland_protocols::wp::primary_selection::zv1::client::zwp_primary_selection_source_v1::ZwpPrimarySelectionSourceV1;

use crate::{Clipboard, ConnectionOps};

use super::data_device::TEXT_MIME_TYPE;
use super::state::WaylandState;

#[derive(Default)]
pub struct CopyAndPaste {
    data_offer: Option<SelectionOffer>,
}

impl std::fmt::Debug for CopyAndPaste {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        fmt.debug_struct("CopyAndPaste")
            .field("data_offer", &self.data_offer.is_some())
            .finish()
    }
}

impl CopyAndPaste {
    pub(super) fn create() -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Default::default()))
    }

    pub(super) fn get_clipboard_data(&mut self, clipboard: Clipboard) -> anyhow::Result<ReadPipe> {
        let conn = crate::Connection::get().unwrap().wayland();
        let wayland_state = conn.wayland_state.borrow();
        let primary_selection = if let Clipboard::PrimarySelection = clipboard {
            wayland_state.primary_selection_device.as_ref()
        } else {
            None
        };

        match primary_selection {
            Some(primary_selection) => {
                let offer = primary_selection
                    .data()
                    .selection_offer()
                    .ok_or_else(|| anyhow!("no primary selection offer"))?;
                let pipe = offer.receive(TEXT_MIME_TYPE.to_string())?;
                Ok(pipe)
            }
            None => {
                let offer = self
                    .data_offer
                    .as_ref()
                    .ok_or_else(|| anyhow!("no data offer"))?;
                let pipe = offer.receive(TEXT_MIME_TYPE.to_string())?;
                Ok(pipe)
            }
        }
    }

    pub(super) fn set_clipboard_data(&mut self, clipboard: Clipboard, data: String) {
        let conn = crate::Connection::get().unwrap().wayland();
        let qh = conn.event_queue.borrow().handle();
        let mut wayland_state = conn.wayland_state.borrow_mut();
        let last_serial = *wayland_state.last_serial.borrow();

        let primary_selection = if let Clipboard::PrimarySelection = clipboard {
            wayland_state.primary_selection_device.as_ref()
        } else {
            None
        };

        match primary_selection {
            Some(primary_selection) => {
                let manager = wayland_state.primary_selection_manager.as_ref().unwrap();
                let source = manager.create_selection_source(&qh, [TEXT_MIME_TYPE]);
                source.set_selection(&primary_selection, last_serial);
                wayland_state
                    .primary_selection_source
                    .replace((source, data));
            }
            None => {
                let data_device = &wayland_state.data_device;
                let source = wayland_state
                    .data_device_manager_state
                    .create_copy_paste_source(&qh, vec![TEXT_MIME_TYPE]);
                source.set_selection(data_device.as_ref().unwrap(), last_serial);
                wayland_state.copy_paste_source.replace((source, data));
            }
        }
    }

    pub(super) fn confirm_selection(&mut self, offer: SelectionOffer) {
        self.data_offer.replace(offer);
    }
}

impl WaylandState {
    pub(super) fn resolve_copy_and_paste(&mut self) -> Option<Arc<Mutex<CopyAndPaste>>> {
        let active_surface_id = self.active_surface_id.borrow();
        if let Some(active_surface_id) = active_surface_id.as_ref() {
            if let Some(pending) = self.surface_to_pending.get(&active_surface_id) {
                Some(Arc::clone(&pending.lock().unwrap().copy_and_paste))
            } else {
                None
            }
        } else {
            None
        }
    }
}

pub(super) fn write_selection_to_pipe(fd: WritePipe, text: &str) {
    if let Err(e) = write_pipe_with_timeout(fd, text.as_bytes()) {
        log::error!("while sending primary selection to pipe: {}", e);
    }
}

fn write_pipe_with_timeout(mut file: WritePipe, data: &[u8]) -> anyhow::Result<()> {
    // set non-blocking I/O on the pipe
    // (adapted from FileDescriptor::set_non_blocking_impl in /filedescriptor/src/unix.rs)
    if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETFL, libc::O_NONBLOCK) } != 0 {
        bail!(
            "failed to change non-blocking mode: {}",
            std::io::Error::last_os_error()
        )
    }

    let mut pfd = libc::pollfd {
        fd: file.as_raw_fd(),
        events: libc::POLLOUT,
        revents: 0,
    };

    let mut buf = data;

    while !buf.is_empty() {
        if unsafe { libc::poll(&mut pfd, 1, 3000) == 1 } {
            match file.write(buf) {
                Ok(size) if size == 0 => {
                    bail!("zero byte write");
                }
                Ok(size) => {
                    buf = &buf[size..];
                }
                Err(e) => bail!("error writing to pipe: {}", e),
            }
        } else {
            bail!("timed out writing to pipe");
        }
    }

    Ok(())
}

impl PrimarySelectionDeviceHandler for WaylandState {
    fn selection(
        &mut self,
        _conn: &wayland_client::Connection,
        _qh: &wayland_client::QueueHandle<Self>,
        _primary_selection_device: &ZwpPrimarySelectionDeviceV1,
    ) {
        // TODO: do we need to do anything here?
    }
}

impl PrimarySelectionSourceHandler for WaylandState {
    fn send_request(
        &mut self,
        _conn: &wayland_client::Connection,
        _qh: &wayland_client::QueueHandle<Self>,
        source: &ZwpPrimarySelectionSourceV1,
        mime: String,
        write_pipe: toolkit::data_device_manager::WritePipe,
    ) {
        if mime != TEXT_MIME_TYPE {
            return;
        };

        if let Some((ps_source, data)) = &self.primary_selection_source {
            if ps_source.inner() != source {
                return;
            }
            write_selection_to_pipe(write_pipe, data);
        }
    }

    fn cancelled(
        &mut self,
        _conn: &wayland_client::Connection,
        _qh: &wayland_client::QueueHandle<Self>,
        source: &ZwpPrimarySelectionSourceV1,
    ) {
        self.primary_selection_source.take();
        source.destroy();
    }
}
