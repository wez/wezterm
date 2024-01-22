use anyhow::{anyhow, Error};
use filedescriptor::{FileDescriptor, Pipe};
use smithay_client_toolkit as toolkit;
use std::os::fd::AsRawFd;
use std::sync::{Arc, Mutex};
use toolkit::reexports::client::protocol::wl_data_offer::WlDataOffer;

use crate::{Clipboard, ConnectionOps};

use super::data_device::TEXT_MIME_TYPE;
use super::state::WaylandState;

#[derive(Default)]
pub struct CopyAndPaste {
    data_offer: Option<WlDataOffer>,
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

    pub(super) fn get_clipboard_data(
        &mut self,
        _clipboard: Clipboard,
    ) -> anyhow::Result<FileDescriptor> {
        // TODO; primary selection
        let offer = self
            .data_offer
            .as_ref()
            .ok_or_else(|| anyhow!("no data offer"))?;
        let pipe = Pipe::new().map_err(Error::msg)?;
        offer.receive(TEXT_MIME_TYPE.to_string(), pipe.write.as_raw_fd());
        Ok(pipe.read)
    }

    pub(super) fn set_clipboard_data(&mut self, _clipboard: Clipboard, data: String) {
        // TODO: primary selection

        let conn = crate::Connection::get().unwrap().wayland();
        let qh = conn.event_queue.borrow().handle();
        let mut wayland_state = conn.wayland_state.borrow_mut();
        let last_serial = *wayland_state.last_serial.borrow();
        let data_device = &wayland_state.data_device;

        let source = wayland_state
            .data_device_manager_state
            .create_copy_paste_source(&qh, vec![TEXT_MIME_TYPE]);
        source.set_selection(data_device.as_ref().unwrap(), last_serial);
        wayland_state.copy_paste_source.replace((source, data));
    }

    pub(super) fn confirm_selection(&mut self, offer: WlDataOffer) {
        self.data_offer.replace(offer);
    }
}

impl WaylandState {
    pub(super) fn resolve_copy_and_paste(&mut self) -> Option<Arc<Mutex<CopyAndPaste>>> {
        let active_surface_id = self.active_surface_id.borrow();
        let active_surface_id = active_surface_id.as_ref().unwrap();
        if let Some(pending) = self.surface_to_pending.get(&active_surface_id) {
            Some(Arc::clone(&pending.lock().unwrap().copy_and_paste))
        } else {
            None
        }
    }
}
