use std::fs::File;
use std::io::Write;
use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd, OwnedFd};

use anyhow::bail;
use filedescriptor::FileDescriptor;
use smithay_client_toolkit::data_device_manager::data_device::{
    DataDevice, DataDeviceDataExt, DataDeviceHandler,
};
use smithay_client_toolkit::data_device_manager::data_offer::DataOfferHandler;
use smithay_client_toolkit::data_device_manager::data_source::DataSourceHandler;
use smithay_client_toolkit::data_device_manager::WritePipe;

use super::state::WaylandState;

pub(super) const TEXT_MIME_TYPE: &str = "text/plain;charset=utf-8";

impl DataDeviceHandler for WaylandState {
    fn enter(
        &mut self,
        _conn: &wayland_client::Connection,
        _qh: &wayland_client::QueueHandle<Self>,
        _data_device: DataDevice,
    ) {
        todo!()
    }

    fn leave(
        &mut self,
        _conn: &wayland_client::Connection,
        _qh: &wayland_client::QueueHandle<Self>,
        _data_device: DataDevice,
    ) {
        todo!()
    }

    fn motion(
        &mut self,
        _conn: &wayland_client::Connection,
        _qh: &wayland_client::QueueHandle<Self>,
        _data_device: DataDevice,
    ) {
        todo!()
    }

    fn selection(
        &mut self,
        _conn: &wayland_client::Connection,
        _qh: &wayland_client::QueueHandle<Self>,
        data_device: DataDevice,
    ) {
        if let Some(offer) = data_device.selection_offer() {
            if let Some(copy_and_paste) = self.resolve_copy_and_paste() {
                copy_and_paste
                    .lock()
                    .unwrap()
                    .confirm_selection(offer.inner().clone());
            }
        }
    }

    fn drop_performed(
        &mut self,
        _conn: &wayland_client::Connection,
        _qh: &wayland_client::QueueHandle<Self>,
        _data_device: DataDevice,
    ) {
        todo!()
    }
}

impl DataOfferHandler for WaylandState {
    fn offer(
        &mut self,
        _conn: &wayland_client::Connection,
        _qh: &wayland_client::QueueHandle<Self>,
        offer: &mut smithay_client_toolkit::data_device_manager::data_offer::DataDeviceOffer,
        mime_type: String,
    ) {
        log::trace!("Received offer with mime type: {mime_type}");
        if mime_type == TEXT_MIME_TYPE {
            offer.accept_mime_type(*self.last_serial.borrow(), Some(mime_type));
        } else {
            // Refuse other mime types
            offer.accept_mime_type(*self.last_serial.borrow(), None);
        }
    }

    fn source_actions(
        &mut self,
        _conn: &wayland_client::Connection,
        _qh: &wayland_client::QueueHandle<Self>,
        _offer: &mut smithay_client_toolkit::data_device_manager::data_offer::DragOffer,
        _actions: wayland_client::protocol::wl_data_device_manager::DndAction,
    ) {
        todo!()
    }

    fn selected_action(
        &mut self,
        _conn: &wayland_client::Connection,
        _qh: &wayland_client::QueueHandle<Self>,
        _offer: &mut smithay_client_toolkit::data_device_manager::data_offer::DragOffer,
        _actions: wayland_client::protocol::wl_data_device_manager::DndAction,
    ) {
        todo!()
    }
}

impl DataSourceHandler for WaylandState {
    fn accept_mime(
        &mut self,
        _conn: &wayland_client::Connection,
        _qh: &wayland_client::QueueHandle<Self>,
        _source: &wayland_client::protocol::wl_data_source::WlDataSource,
        _mime: Option<String>,
    ) {
        todo!()
    }

    fn send_request(
        &mut self,
        _conn: &wayland_client::Connection,
        _qh: &wayland_client::QueueHandle<Self>,
        source: &wayland_client::protocol::wl_data_source::WlDataSource,
        mime: String,
        fd: WritePipe,
    ) {
        if mime != TEXT_MIME_TYPE {
            return;
        }

        if let Some((cp_source, data)) = &self.copy_paste_source {
            if cp_source.inner() != source {
                return;
            }
            let fd = unsafe { FileDescriptor::from_raw_fd(fd.into_raw_fd()) };
            write_selection_to_pipe(fd, data);
        }
    }

    fn cancelled(
        &mut self,
        _conn: &wayland_client::Connection,
        _qh: &wayland_client::QueueHandle<Self>,
        _source: &wayland_client::protocol::wl_data_source::WlDataSource,
    ) {
        todo!()
    }

    fn dnd_dropped(
        &mut self,
        _conn: &wayland_client::Connection,
        _qh: &wayland_client::QueueHandle<Self>,
        _source: &wayland_client::protocol::wl_data_source::WlDataSource,
    ) {
        todo!()
    }

    fn dnd_finished(
        &mut self,
        _conn: &wayland_client::Connection,
        _qh: &wayland_client::QueueHandle<Self>,
        _source: &wayland_client::protocol::wl_data_source::WlDataSource,
    ) {
        todo!()
    }

    fn action(
        &mut self,
        _conn: &wayland_client::Connection,
        _qh: &wayland_client::QueueHandle<Self>,
        _source: &wayland_client::protocol::wl_data_source::WlDataSource,
        _action: wayland_client::protocol::wl_data_device_manager::DndAction,
    ) {
        todo!()
    }
}

fn write_selection_to_pipe(fd: FileDescriptor, text: &str) {
    if let Err(e) = write_pipe_with_timeout(fd, text.as_bytes()) {
        log::error!("while sending primary selection to pipe: {}", e);
    }
}

fn write_pipe_with_timeout(mut file: FileDescriptor, data: &[u8]) -> anyhow::Result<()> {
    file.set_non_blocking(true)?;
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
