use super::copy_and_paste::*;
use crate::input::*;
use crate::os::wayland::connection::WaylandConnection;
use failure::Fallible;
use smithay_client_toolkit as toolkit;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use toolkit::reexports::client::protocol::wl_data_device::{
    Event as DataDeviceEvent, WlDataDevice,
};
use toolkit::reexports::client::protocol::wl_data_offer::{Event as DataOfferEvent, WlDataOffer};
use toolkit::reexports::client::protocol::wl_pointer::{
    self, Axis, AxisSource, Event as PointerEvent,
};
use toolkit::reexports::client::protocol::wl_surface::WlSurface;
use wayland_client::protocol::wl_data_device_manager::WlDataDeviceManager;
use wayland_client::protocol::wl_seat::WlSeat;

#[derive(Default)]
struct Inner {
    active_surface_id: u32,
    surface_to_pending: HashMap<u32, Arc<Mutex<PendingMouse>>>,
}

impl Inner {
    fn handle_event(&mut self, evt: PointerEvent) {
        if let PointerEvent::Enter { surface, .. } = &evt {
            self.active_surface_id = surface.as_ref().id();
        }
        let evt: SendablePointerEvent = evt.into();
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
                id.implement_closure(
                    {
                        let inner = Arc::clone(inner);
                        move |event, offer| {
                            let mut inner = inner.lock().unwrap();
                            inner.route_data_offer(event, offer);
                        }
                    },
                    (),
                );
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

#[derive(Clone)]
pub struct PointerDispatcher {
    inner: Arc<Mutex<Inner>>,
    pub(crate) data_device: WlDataDevice,
}

#[derive(Clone)]
pub struct PendingMouse {
    window_id: usize,
    copy_and_paste: Arc<Mutex<CopyAndPaste>>,
    surface_coords: Option<(f64, f64)>,
    button: Vec<(MousePress, DebuggableButtonState)>,
    scroll: Option<(f64, f64)>,
}

impl PendingMouse {
    pub fn create(window_id: usize, copy_and_paste: &Arc<Mutex<CopyAndPaste>>) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            window_id,
            copy_and_paste: Arc::clone(copy_and_paste),
            button: vec![],
            scroll: None,
            surface_coords: None,
        }))
    }

    // Return true if we need to queue up a call to act on the event,
    // false if we think there is already a pending event
    pub fn queue(&mut self, evt: SendablePointerEvent) -> bool {
        match evt {
            SendablePointerEvent::Enter { serial, .. } => {
                self.copy_and_paste
                    .lock()
                    .unwrap()
                    .update_last_serial(serial);
                false
            }
            SendablePointerEvent::Motion {
                surface_x,
                surface_y,
                ..
            } => {
                let changed = self.surface_coords.is_none();
                self.surface_coords.replace((surface_x, surface_y));
                changed
            }
            SendablePointerEvent::Button {
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
            SendablePointerEvent::Axis {
                axis: Axis::VerticalScroll,
                value,
                ..
            } => {
                let changed = self.scroll.is_none();
                let (x, y) = self.scroll.take().unwrap_or((0., 0.));
                self.scroll.replace((x, y + value));
                changed
            }
            SendablePointerEvent::Axis {
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

    pub fn next_button(pending: &Arc<Mutex<Self>>) -> Option<(MousePress, DebuggableButtonState)> {
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
}

impl PointerDispatcher {
    pub fn register(seat: &WlSeat, dev_mgr: &WlDataDeviceManager) -> Fallible<Self> {
        let inner = Arc::new(Mutex::new(Inner::default()));
        seat.get_pointer({
            let inner = Arc::clone(&inner);
            move |ptr| {
                ptr.implement_closure(
                    {
                        let inner = Arc::clone(&inner);
                        move |evt, _| {
                            inner.lock().unwrap().handle_event(evt);
                        }
                    },
                    (),
                )
            }
        })
        .map_err(|_| failure::format_err!("Failed to configure pointer callback"))?;

        let data_device = dev_mgr
            .get_data_device(seat, {
                let inner = Arc::clone(&inner);
                move |device| {
                    device.implement_closure(
                        {
                            let inner = Arc::clone(&inner);
                            move |event, _device| {
                                inner.lock().unwrap().handle_data_event(event, &inner);
                            }
                        },
                        (),
                    )
                }
            })
            .map_err(|_| failure::format_err!("Failed to configure data_device"))?;

        Ok(Self { inner, data_device })
    }

    pub fn add_window(&self, surface: &WlSurface, pending: &Arc<Mutex<PendingMouse>>) {
        let mut inner = self.inner.lock().unwrap();
        inner
            .surface_to_pending
            .insert(surface.as_ref().id(), Arc::clone(pending));
    }
}

#[derive(Clone, Debug, Copy, PartialEq, Eq)]
pub enum DebuggableButtonState {
    Released,
    Pressed,
}

impl From<wl_pointer::ButtonState> for DebuggableButtonState {
    fn from(state: wl_pointer::ButtonState) -> DebuggableButtonState {
        match state {
            wl_pointer::ButtonState::Released => Self::Released,
            wl_pointer::ButtonState::Pressed => Self::Pressed,
            _ => unreachable!(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum SendablePointerEvent {
    Enter {
        serial: u32,
        surface_x: f64,
        surface_y: f64,
    },
    Leave {
        serial: u32,
    },
    Motion {
        time: u32,
        surface_x: f64,
        surface_y: f64,
    },
    Button {
        serial: u32,
        time: u32,
        button: u32,
        state: DebuggableButtonState,
    },
    Axis {
        time: u32,
        axis: Axis,
        value: f64,
    },
    Frame,
    AxisSource {
        axis_source: AxisSource,
    },
    AxisStop {
        time: u32,
        axis: Axis,
    },
    AxisDiscrete {
        axis: Axis,
        discrete: i32,
    },
}

impl From<PointerEvent> for SendablePointerEvent {
    fn from(event: PointerEvent) -> Self {
        match event {
            PointerEvent::Enter {
                serial,
                surface_x,
                surface_y,
                ..
            } => SendablePointerEvent::Enter {
                serial,
                surface_x,
                surface_y,
            },
            PointerEvent::Leave { serial, .. } => SendablePointerEvent::Leave { serial },
            PointerEvent::Motion {
                time,
                surface_x,
                surface_y,
            } => SendablePointerEvent::Motion {
                time,
                surface_x,
                surface_y,
            },
            PointerEvent::Button {
                serial,
                time,
                button,
                state,
                ..
            } => SendablePointerEvent::Button {
                serial,
                time,
                button,
                state: state.into(),
            },
            PointerEvent::Axis { time, axis, value } => {
                SendablePointerEvent::Axis { time, axis, value }
            }
            PointerEvent::Frame => SendablePointerEvent::Frame,
            PointerEvent::AxisSource { axis_source, .. } => {
                SendablePointerEvent::AxisSource { axis_source }
            }
            PointerEvent::AxisStop { axis, time } => SendablePointerEvent::AxisStop { axis, time },
            PointerEvent::AxisDiscrete { axis, discrete } => {
                SendablePointerEvent::AxisDiscrete { axis, discrete }
            }
            _ => unreachable!(),
        }
    }
}
