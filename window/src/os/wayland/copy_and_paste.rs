use anyhow::{anyhow, Error, bail};
use filedescriptor::{FileDescriptor, Pipe};
use smithay_client_toolkit as toolkit;
use toolkit::globals::GlobalData;
use wayland_client::{Dispatch, event_created_child};
use wayland_client::globals::{GlobalList, BindError};
use wayland_protocols::wp::primary_selection::zv1::client::zwp_primary_selection_device_manager_v1::ZwpPrimarySelectionDeviceManagerV1;
use wayland_protocols::wp::primary_selection::zv1::client::zwp_primary_selection_device_v1::{ZwpPrimarySelectionDeviceV1, self, Event as PrimarySelectionDeviceEvent};
use wayland_protocols::wp::primary_selection::zv1::client::zwp_primary_selection_offer_v1::{ZwpPrimarySelectionOfferV1, Event as PrimarySelectionOfferEvent};
use wayland_protocols::wp::primary_selection::zv1::client::zwp_primary_selection_source_v1::{ZwpPrimarySelectionSourceV1, Event as PrimarySelectionSourceEvent};
use std::io::Write;
use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd};
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
        clipboard: Clipboard,
    ) -> anyhow::Result<FileDescriptor> {
        let conn = crate::Connection::get().unwrap().wayland();
        let wayland_state = conn.wayland_state.borrow();
        let primary_selection = if let Clipboard::PrimarySelection = clipboard {
            wayland_state.primary_selection_manager.as_ref()
        } else {
            None
        };

        match primary_selection {
            Some(primary_selection) => {
                let inner = primary_selection.inner.lock().unwrap();
                let offer = inner
                    .offer
                    .as_ref()
                    .ok_or_else(|| anyhow!("no primary selection offer"))?;
                let pipe = Pipe::new().map_err(Error::msg)?;
                offer.receive(TEXT_MIME_TYPE.to_string(), pipe.write.as_raw_fd());
                Ok(pipe.read)
            }
            None => {
                let offer = self
                    .data_offer
                    .as_ref()
                    .ok_or_else(|| anyhow!("no data offer"))?;
                let pipe = Pipe::new().map_err(Error::msg)?;
                offer.receive(TEXT_MIME_TYPE.to_string(), pipe.write.as_raw_fd());
                Ok(pipe.read)
            }
        }
    }

    pub(super) fn set_clipboard_data(&mut self, clipboard: Clipboard, data: String) {
        let conn = crate::Connection::get().unwrap().wayland();
        let qh = conn.event_queue.borrow().handle();
        let mut wayland_state = conn.wayland_state.borrow_mut();
        let last_serial = *wayland_state.last_serial.borrow();

        let primary_selection = if let Clipboard::PrimarySelection = clipboard {
            wayland_state.primary_selection_manager.as_ref()
        } else {
            None
        };

        match primary_selection {
            Some(primary_selection) => {
                let manager = &primary_selection.manager;
                let selection_device = wayland_state.primary_select_device.as_ref().unwrap();
                let source = manager.create_source(&qh, PrimarySelectionManagerData::default());
                source.offer(TEXT_MIME_TYPE.to_string());
                selection_device.set_selection(Some(&source), last_serial);
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

pub(super) fn write_selection_to_pipe(fd: FileDescriptor, text: &str) {
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

// Smithay has their own primary selection handler in 0.18
// Some code borrowed from https://github.com/Smithay/client-toolkit/commit/4a5c4f59f640bc588a55277261bbed1bd2abea98
pub(super) struct PrimarySelectionManagerState {
    pub(super) manager: ZwpPrimarySelectionDeviceManagerV1,
    inner: Mutex<PrimaryInner>,
}

#[derive(Default, Debug)]
struct PrimaryInner {
    pending_offer: Option<ZwpPrimarySelectionOfferV1>,
    offer: Option<ZwpPrimarySelectionOfferV1>,
    valid_mime: bool,
}

#[derive(Default)]
pub(super) struct PrimarySelectionManagerData {}

impl PrimarySelectionManagerState {
    pub(super) fn bind(
        globals: &GlobalList,
        queue_handle: &wayland_client::QueueHandle<WaylandState>,
    ) -> Result<Self, BindError> {
        let manager = globals.bind(queue_handle, 1..=1, GlobalData)?;
        Ok(Self {
            manager,
            inner: Mutex::new(PrimaryInner::default()),
        })
    }
}

impl Dispatch<ZwpPrimarySelectionDeviceManagerV1, GlobalData, WaylandState>
    for PrimarySelectionManagerState
{
    fn event(
        _state: &mut WaylandState,
        _proxy: &ZwpPrimarySelectionDeviceManagerV1,
        _event: <ZwpPrimarySelectionDeviceManagerV1 as wayland_client::Proxy>::Event,
        _data: &GlobalData,
        _conn: &wayland_client::Connection,
        _qhandle: &wayland_client::QueueHandle<WaylandState>,
    ) {
        unreachable!("primary selection manager has no events");
    }
}

impl Dispatch<ZwpPrimarySelectionSourceV1, PrimarySelectionManagerData, WaylandState>
    for PrimarySelectionManagerState
{
    fn event(
        state: &mut WaylandState,
        source: &ZwpPrimarySelectionSourceV1,
        event: <ZwpPrimarySelectionSourceV1 as wayland_client::Proxy>::Event,
        _data: &PrimarySelectionManagerData,
        _conn: &wayland_client::Connection,
        _qhandle: &wayland_client::QueueHandle<WaylandState>,
    ) {
        match event {
            PrimarySelectionSourceEvent::Send { mime_type, fd } => {
                if mime_type != TEXT_MIME_TYPE {
                    return;
                };

                if let Some((ps_source, data)) = &state.primary_selection_source {
                    if ps_source != source {
                        return;
                    }
                    let fd = unsafe { FileDescriptor::from_raw_fd(fd.into_raw_fd()) };
                    write_selection_to_pipe(fd, data);
                }
            }
            PrimarySelectionSourceEvent::Cancelled => {
                state.primary_selection_source.take();
                source.destroy();
            }
            _ => unreachable!(),
        }
    }
}

impl Dispatch<ZwpPrimarySelectionOfferV1, PrimarySelectionManagerData, WaylandState>
    for PrimarySelectionManagerState
{
    fn event(
        state: &mut WaylandState,
        _proxy: &ZwpPrimarySelectionOfferV1,
        event: <ZwpPrimarySelectionOfferV1 as wayland_client::Proxy>::Event,
        _data: &PrimarySelectionManagerData,
        _conn: &wayland_client::Connection,
        _qhandle: &wayland_client::QueueHandle<WaylandState>,
    ) {
        match event {
            PrimarySelectionOfferEvent::Offer { mime_type } => {
                if mime_type == TEXT_MIME_TYPE {
                    let mgr = state.primary_selection_manager.as_ref().unwrap();
                    let mut inner = mgr.inner.lock().unwrap();
                    inner.valid_mime = true;
                }
            }
            _ => unreachable!(),
        }
    }
}

impl Dispatch<ZwpPrimarySelectionDeviceV1, PrimarySelectionManagerData, WaylandState>
    for PrimarySelectionManagerState
{
    event_created_child!(WaylandState, ZwpPrimarySelectionDeviceV1, [
        zwp_primary_selection_device_v1::EVT_DATA_OFFER_OPCODE => (ZwpPrimarySelectionOfferV1, PrimarySelectionManagerData::default())
    ]);

    fn event(
        state: &mut WaylandState,
        _primary_selection_device: &ZwpPrimarySelectionDeviceV1,
        event: <ZwpPrimarySelectionDeviceV1 as wayland_client::Proxy>::Event,
        _data: &PrimarySelectionManagerData,
        _conn: &wayland_client::Connection,
        _qhandle: &wayland_client::QueueHandle<WaylandState>,
    ) {
        let psm = state.primary_selection_manager.as_ref().unwrap();
        let mut inner = psm.inner.lock().unwrap();
        match event {
            PrimarySelectionDeviceEvent::DataOffer { offer } => {
                inner.pending_offer = Some(offer);
            }
            PrimarySelectionDeviceEvent::Selection { id } => {
                if !inner.valid_mime {
                    return;
                }

                if let Some(offer) = inner.offer.take() {
                    offer.destroy();
                }
                if id == inner.pending_offer {
                    inner.offer = inner.pending_offer.take();
                } else {
                    // Remove the pending offer, assign the new delivered one.
                    if let Some(offer) = inner.pending_offer.take() {
                        offer.destroy()
                    }

                    inner.offer = id;
                }
            }
            _ => unreachable!(),
        }
    }
}
