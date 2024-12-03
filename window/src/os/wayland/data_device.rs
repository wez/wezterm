use smithay_client_toolkit::data_device_manager::data_device::DataDeviceHandler;
use smithay_client_toolkit::data_device_manager::data_offer::DataOfferHandler;
use smithay_client_toolkit::data_device_manager::data_source::DataSourceHandler;
use smithay_client_toolkit::data_device_manager::WritePipe;
use smithay_client_toolkit::reexports::client::protocol::wl_data_device::WlDataDevice;
use wayland_client::protocol::wl_data_device_manager::DndAction;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::Proxy;

use crate::wayland::drag_and_drop::SurfaceAndOffer;
use crate::wayland::pointer::PointerUserData;
use crate::wayland::SurfaceUserData;

use super::copy_and_paste::write_selection_to_pipe;
use super::drag_and_drop::{DragAndDrop, SurfaceAndPipe};
use super::state::WaylandState;

pub(super) const TEXT_MIME_TYPE: &str = "text/plain;charset=utf-8";
pub(super) const URI_MIME_TYPE: &str = "text/uri-list";

impl DataDeviceHandler for WaylandState {
    fn enter(
        &mut self,
        _conn: &wayland_client::Connection,
        _qh: &wayland_client::QueueHandle<Self>,
        data_device: &WlDataDevice,
        _x: f64,
        _y: f64,
        _surface: &WlSurface,
    ) {
        let data = match self.data_device {
            Some(ref dv) if dv.inner() == data_device => dv.data(),
            _ => {
                log::warn!("No existing device manager for {:?}", data_device);
                return;
            }
        };

        let offer = data.drag_offer().unwrap();

        offer.with_mime_types(|mime_types| {
            log::trace!(
                "Data offer entered: {:?}, mime_types: {:?}",
                offer,
                mime_types
            );

            if let Some(mime) = mime_types.iter().find(|s| *s == URI_MIME_TYPE) {
                offer.accept_mime_type(*self.last_serial.borrow(), Some(mime.clone()));
            }
        });

        offer.set_actions(DndAction::None | DndAction::Copy, DndAction::None);

        let pointer = self.pointer.as_mut().unwrap();
        let mut pstate = pointer
            .pointer()
            .data::<PointerUserData>()
            .unwrap()
            .state
            .lock()
            .unwrap();

        let window_id = SurfaceUserData::from_wl(&offer.surface).window_id;

        pstate.drag_and_drop.offer = Some(SurfaceAndOffer { window_id, offer });
    }

    fn leave(
        &mut self,
        _conn: &wayland_client::Connection,
        _qh: &wayland_client::QueueHandle<Self>,
        _data_device: &WlDataDevice,
    ) {
        let pointer = self.pointer.as_mut().unwrap();
        let mut pstate = pointer
            .pointer()
            .data::<PointerUserData>()
            .unwrap()
            .state
            .lock()
            .unwrap();
        if let Some(SurfaceAndOffer { offer, .. }) = pstate.drag_and_drop.offer.take() {
            offer.destroy();
        }
    }

    fn motion(
        &mut self,
        _conn: &wayland_client::Connection,
        _qh: &wayland_client::QueueHandle<Self>,
        _data_device: &WlDataDevice,
        _x: f64,
        _y: f64,
    ) {
    }

    fn selection(
        &mut self,
        _conn: &wayland_client::Connection,
        _qh: &wayland_client::QueueHandle<Self>,
        data_device: &WlDataDevice,
    ) {
        let offer = match self.data_device {
            Some(ref dv) if dv.inner() == data_device => dv.data().selection_offer(),
            _ => {
                return;
            }
        };
        if let Some(offer) = offer {
            if !offer.with_mime_types(|mime_types| mime_types.iter().any(|s| s == TEXT_MIME_TYPE)) {
                return;
            }

            if let Some(copy_and_paste) = self.resolve_copy_and_paste() {
                copy_and_paste.lock().unwrap().confirm_selection(offer);
            }
        }
    }

    fn drop_performed(
        &mut self,
        _conn: &wayland_client::Connection,
        _qh: &wayland_client::QueueHandle<Self>,
        _data_device: &WlDataDevice,
    ) {
        let pointer = self.pointer.as_mut().unwrap();
        let mut pstate = pointer
            .pointer()
            .data::<PointerUserData>()
            .unwrap()
            .state
            .lock()
            .unwrap();
        let drag_and_drop = &mut pstate.drag_and_drop;
        if let Some(SurfaceAndPipe { window_id, read }) = drag_and_drop.create_pipe_for_drop() {
            std::thread::spawn(move || {
                if let Some(paths) = DragAndDrop::read_paths_from_pipe(read) {
                    DragAndDrop::dispatch_dropped_files(window_id, paths);
                }
            });
        }
        // if let Some(SurfaceAndOffer { offer, .. }) = pstate.drag_and_drop.offer.take() {
    }
}

impl DataOfferHandler for WaylandState {
    // Ignore drag and drop events
    fn source_actions(
        &mut self,
        _conn: &wayland_client::Connection,
        _qh: &wayland_client::QueueHandle<Self>,
        _offer: &mut smithay_client_toolkit::data_device_manager::data_offer::DragOffer,
        _actions: wayland_client::protocol::wl_data_device_manager::DndAction,
    ) {
    }

    fn selected_action(
        &mut self,
        _conn: &wayland_client::Connection,
        _qh: &wayland_client::QueueHandle<Self>,
        _offer: &mut smithay_client_toolkit::data_device_manager::data_offer::DragOffer,
        _actions: wayland_client::protocol::wl_data_device_manager::DndAction,
    ) {
    }
}

// We seem to ignore all events other than sending_request and cancelled
impl DataSourceHandler for WaylandState {
    fn accept_mime(
        &mut self,
        _conn: &wayland_client::Connection,
        _qh: &wayland_client::QueueHandle<Self>,
        _source: &wayland_client::protocol::wl_data_source::WlDataSource,
        _mime: Option<String>,
    ) {
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
            write_selection_to_pipe(fd, data);
        }
    }

    fn cancelled(
        &mut self,
        _conn: &wayland_client::Connection,
        _qh: &wayland_client::QueueHandle<Self>,
        source: &wayland_client::protocol::wl_data_source::WlDataSource,
    ) {
        self.copy_paste_source.take();
        source.destroy();
    }

    fn dnd_dropped(
        &mut self,
        _conn: &wayland_client::Connection,
        _qh: &wayland_client::QueueHandle<Self>,
        _source: &wayland_client::protocol::wl_data_source::WlDataSource,
    ) {
    }

    fn dnd_finished(
        &mut self,
        _conn: &wayland_client::Connection,
        _qh: &wayland_client::QueueHandle<Self>,
        _source: &wayland_client::protocol::wl_data_source::WlDataSource,
    ) {
    }

    fn action(
        &mut self,
        _conn: &wayland_client::Connection,
        _qh: &wayland_client::QueueHandle<Self>,
        _source: &wayland_client::protocol::wl_data_source::WlDataSource,
        _action: wayland_client::protocol::wl_data_device_manager::DndAction,
    ) {
    }
}
