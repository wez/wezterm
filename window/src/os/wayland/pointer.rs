use super::copy_and_paste::*;
use crate::os::wayland::connection::WaylandConnection;
use smithay_client_toolkit as toolkit;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use toolkit::primary_selection::{PrimarySelectionDevice, PrimarySelectionDeviceManager};
use toolkit::reexports::client::protocol::wl_data_device::{
    Event as DataDeviceEvent, WlDataDevice,
};
use toolkit::reexports::client::protocol::wl_data_offer::{Event as DataOfferEvent, WlDataOffer};
use toolkit::reexports::client::protocol::wl_pointer::{Axis, ButtonState, Event as PointerEvent};
use toolkit::reexports::client::protocol::wl_surface::WlSurface;
use toolkit::seat::pointer::{ThemeManager, ThemeSpec, ThemedPointer};
use wayland_client::protocol::wl_compositor::WlCompositor;
use wayland_client::protocol::wl_data_device_manager::WlDataDeviceManager;
use wayland_client::protocol::wl_seat::WlSeat;
use wayland_client::protocol::wl_shm::WlShm;
use wayland_client::{Attached, Main};
use wezterm_input_types::*;

#[derive(Default)]
struct Inner {
    active_surface_id: u32,
    surface_to_pending: HashMap<u32, Arc<Mutex<PendingMouse>>>,
    serial: u32,
}

impl Inner {
    fn handle_event(&mut self, evt: PointerEvent) {
        if let PointerEvent::Enter { surface, .. } = &evt {
            self.active_surface_id = surface.as_ref().id();
        }
        if let Some(serial) = event_serial(&evt) {
            self.serial = serial;
        }
        if let Some(pending) = self.surface_to_pending.get(&self.active_surface_id) {
            let mut pending = pending.lock().unwrap();
            if pending.queue(evt) {
                WaylandConnection::with_window_inner(pending.window_id, move |inner| {
                    inner.dispatch_pending_mouse();
                    Ok(())
                });
            }
        }
    }

    fn resolve_copy_and_paste(&mut self) -> Option<Arc<Mutex<CopyAndPaste>>> {
        if let Some(pending) = self.surface_to_pending.get(&self.active_surface_id) {
            Some(Arc::clone(&pending.lock().unwrap().copy_and_paste))
        } else {
            None
        }
    }

    fn route_data_offer(&mut self, event: DataOfferEvent, offer: WlDataOffer) {
        if let Some(copy_and_paste) = self.resolve_copy_and_paste() {
            copy_and_paste
                .lock()
                .unwrap()
                .handle_data_offer(event, offer);
        }
    }

    fn handle_data_event(&mut self, event: DataDeviceEvent, inner: &Arc<Mutex<Self>>) {
        match event {
            DataDeviceEvent::DataOffer { id } => {
                id.quick_assign({
                    let inner = Arc::clone(inner);
                    move |offer, event, _dispatch_data| {
                        let mut inner = inner.lock().unwrap();
                        inner.route_data_offer(event, offer.detach());
                    }
                });
            }
            DataDeviceEvent::Enter { .. }
            | DataDeviceEvent::Leave { .. }
            | DataDeviceEvent::Motion { .. }
            | DataDeviceEvent::Drop => {}

            DataDeviceEvent::Selection { id } => {
                if let Some(offer) = id {
                    if let Some(copy_and_paste) = self.resolve_copy_and_paste() {
                        copy_and_paste.lock().unwrap().confirm_selection(offer);
                    }
                }
            }
            _ => {}
        }
    }
}

pub struct PointerDispatcher {
    inner: Arc<Mutex<Inner>>,
    dev_mgr: Attached<WlDataDeviceManager>,
    pub(crate) data_device: Main<WlDataDevice>,
    selection_manager: Option<PrimarySelectionDeviceManager>,
    pub(crate) primary_selection_device: Option<PrimarySelectionDevice>,
    auto_pointer: ThemedPointer,
    #[allow(dead_code)]
    themer: ThemeManager,
    pub(crate) seat: WlSeat,
}

#[derive(Clone, Debug)]
pub struct PendingMouse {
    window_id: usize,
    copy_and_paste: Arc<Mutex<CopyAndPaste>>,
    surface_coords: Option<(f64, f64)>,
    button: Vec<(MousePress, ButtonState)>,
    scroll: Option<(f64, f64)>,
    in_window: bool,
}

impl PendingMouse {
    pub fn create(window_id: usize, copy_and_paste: &Arc<Mutex<CopyAndPaste>>) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            window_id,
            copy_and_paste: Arc::clone(copy_and_paste),
            button: vec![],
            scroll: None,
            surface_coords: None,
            in_window: false,
        }))
    }

    // Return true if we need to queue up a call to act on the event,
    // false if we think there is already a pending event
    pub fn queue(&mut self, evt: PointerEvent) -> bool {
        match evt {
            PointerEvent::Enter { serial, .. } => {
                self.copy_and_paste
                    .lock()
                    .unwrap()
                    .update_last_serial(serial);
                self.in_window = true;
                false
            }
            PointerEvent::Leave { .. } => {
                let changed = self.in_window;
                self.surface_coords = None;
                self.in_window = false;
                changed
            }
            PointerEvent::Motion {
                surface_x,
                surface_y,
                ..
            } => {
                let changed = self.surface_coords.is_none();
                self.surface_coords.replace((surface_x, surface_y));
                changed
            }
            PointerEvent::Button {
                button,
                state,
                serial,
                ..
            } => {
                self.copy_and_paste
                    .lock()
                    .unwrap()
                    .update_last_serial(serial);
                fn linux_button(b: u32) -> Option<MousePress> {
                    // See BTN_LEFT and friends in <linux/input-event-codes.h>
                    match b {
                        0x110 => Some(MousePress::Left),
                        0x111 => Some(MousePress::Right),
                        0x112 => Some(MousePress::Middle),
                        _ => None,
                    }
                }
                let button = match linux_button(button) {
                    Some(button) => button,
                    None => return false,
                };
                let changed = self.button.is_empty();
                self.button.push((button, state));
                changed
            }
            PointerEvent::Axis {
                axis: Axis::VerticalScroll,
                value,
                ..
            } => {
                let changed = self.scroll.is_none();
                let (x, y) = self.scroll.take().unwrap_or((0., 0.));
                self.scroll.replace((x, y + value));
                changed
            }
            PointerEvent::Axis {
                axis: Axis::HorizontalScroll,
                value,
                ..
            } => {
                let changed = self.scroll.is_none();
                let (x, y) = self.scroll.take().unwrap_or((0., 0.));
                self.scroll.replace((x + value, y));
                changed
            }
            _ => false,
        }
    }

    pub fn next_button(pending: &Arc<Mutex<Self>>) -> Option<(MousePress, ButtonState)> {
        let mut pending = pending.lock().unwrap();
        if pending.button.is_empty() {
            None
        } else {
            Some(pending.button.remove(0))
        }
    }

    pub fn coords(pending: &Arc<Mutex<Self>>) -> Option<(f64, f64)> {
        pending.lock().unwrap().surface_coords.take()
    }

    pub fn scroll(pending: &Arc<Mutex<Self>>) -> Option<(f64, f64)> {
        pending.lock().unwrap().scroll.take()
    }

    pub fn in_window(pending: &Arc<Mutex<Self>>) -> bool {
        pending.lock().unwrap().in_window
    }
}

pub fn make_theme_manager(
    compositor: Attached<WlCompositor>,
    shm: Attached<WlShm>,
) -> ThemeManager {
    let config = config::configuration();
    let name = config
        .xcursor_theme
        .as_ref()
        .map(|s| s.to_string())
        .or_else(|| std::env::var("XCURSOR_THEME").ok())
        .unwrap_or_else(|| "default".to_string());
    let size = match config.xcursor_size {
        Some(size) => size,
        None => match std::env::var("XCURSOR_SIZE").ok() {
            Some(size_str) => size_str.parse().ok(),
            None => None,
        }
        .unwrap_or(24),
    };

    let theme = ThemeSpec::Precise { name: &name, size };

    ThemeManager::init(theme, compositor, shm)
}

impl PointerDispatcher {
    pub fn register(
        seat: &WlSeat,
        compositor: Attached<WlCompositor>,
        shm: Attached<WlShm>,
        dev_mgr: Attached<WlDataDeviceManager>,
        selection_manager: Option<PrimarySelectionDeviceManager>,
    ) -> anyhow::Result<Self> {
        let inner = Arc::new(Mutex::new(Inner::default()));
        let pointer = seat.get_pointer();
        pointer.quick_assign({
            let inner = Arc::clone(&inner);
            move |_, evt, _| {
                inner.lock().unwrap().handle_event(evt);
            }
        });

        let themer = make_theme_manager(compositor, shm);
        let auto_pointer = themer.theme_pointer(pointer.detach());

        let data_device = dev_mgr.get_data_device(seat);
        data_device.quick_assign({
            let inner = Arc::clone(&inner);
            move |_device, event, _| {
                inner.lock().unwrap().handle_data_event(event, &inner);
            }
        });

        let primary_selection_device = selection_manager
            .as_ref()
            .map(|m| PrimarySelectionDevice::init_for_seat(&m, seat));

        Ok(Self {
            inner,
            dev_mgr,
            data_device,
            selection_manager,
            primary_selection_device,
            themer,
            auto_pointer,
            seat: seat.clone(),
        })
    }

    pub fn seat_changed(&mut self, seat: &WlSeat) {
        let inner = Arc::clone(&self.inner);

        let pointer = seat.get_pointer();
        pointer.quick_assign({
            let inner = Arc::clone(&inner);
            move |_, evt, _| {
                inner.lock().unwrap().handle_event(evt);
            }
        });
        let data_device = self.dev_mgr.get_data_device(seat);
        data_device.quick_assign({
            let inner = Arc::clone(&inner);
            move |_device, event, _| {
                inner.lock().unwrap().handle_data_event(event, &inner);
            }
        });

        let primary_selection_device = self
            .selection_manager
            .as_ref()
            .map(|m| PrimarySelectionDevice::init_for_seat(&m, seat));

        self.data_device = data_device;
        self.primary_selection_device = primary_selection_device;
        self.seat = seat.clone();
    }

    pub fn add_window(&self, surface: &WlSurface, pending: &Arc<Mutex<PendingMouse>>) {
        let mut inner = self.inner.lock().unwrap();
        inner
            .surface_to_pending
            .insert(surface.as_ref().id(), Arc::clone(pending));
        if inner.active_surface_id == 0 {
            inner.active_surface_id = surface.as_ref().id();
        }
    }

    pub fn set_cursor(&self, name: &str, serial: Option<u32>) {
        let inner = self.inner.lock().unwrap();
        let serial = serial.unwrap_or(inner.serial);
        if let Err(err) = self.auto_pointer.set_cursor(name, Some(serial)) {
            log::error!("Unable to set cursor to {}: {:#}", name, err);
        }
    }
}

fn event_serial(event: &PointerEvent) -> Option<u32> {
    Some(*match event {
        PointerEvent::Enter { serial, .. } => serial,
        PointerEvent::Leave { serial, .. } => serial,
        PointerEvent::Button { serial, .. } => serial,
        _ => return None,
    })
}
