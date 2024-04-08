use std::cell::RefCell;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use smithay_client_toolkit::compositor::SurfaceData;
use smithay_client_toolkit::reexports::csd_frame::{DecorationsFrame, FrameClick};
use smithay_client_toolkit::seat::pointer::{
    PointerData, PointerDataExt, PointerEvent, PointerEventKind, PointerHandler,
};
use wayland_client::backend::ObjectId;
use wayland_client::protocol::wl_pointer::{ButtonState, WlPointer};
use wayland_client::protocol::wl_seat::WlSeat;
use wayland_client::{Connection, Proxy, QueueHandle};
use wezterm_input_types::MousePress;

use crate::wayland::SurfaceUserData;

use super::copy_and_paste::CopyAndPaste;
use super::drag_and_drop::DragAndDrop;
use super::state::WaylandState;
use super::WaylandConnection;

impl PointerHandler for WaylandState {
    fn pointer_frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        pointer: &WlPointer,
        events: &[PointerEvent],
    ) {
        let mut pstate = pointer
            .data::<PointerUserData>()
            .unwrap()
            .state
            .lock()
            .unwrap();

        for evt in events {
            if let PointerEventKind::Enter { .. } = &evt.kind {
                let surface_id = evt.surface.id();
                self.active_surface_id = RefCell::new(Some(surface_id.clone()));
                pstate.active_surface_id = Some(surface_id);
            }
            if let Some(serial) = event_serial(&evt) {
                *self.last_serial.borrow_mut() = serial;
                pstate.serial = serial;
            }
            if let Some(pending) = self
                .surface_to_pending
                .get(&self.active_surface_id.borrow().as_ref().unwrap())
            {
                let mut pending = pending.lock().unwrap();
                if pending.queue(evt) {
                    WaylandConnection::with_window_inner(pending.window_id, move |inner| {
                        inner.dispatch_pending_mouse();
                        Ok(())
                    });
                }
            }
        }
        self.pointer_window_frame(pointer, events);
    }
}

pub(super) struct PointerUserData {
    pub(super) pdata: PointerData,
    pub(super) state: Mutex<PointerState>,
}

impl PointerUserData {
    pub(super) fn new(seat: WlSeat) -> Self {
        Self {
            pdata: PointerData::new(seat),
            state: Default::default(),
        }
    }
}

#[derive(Default)]
pub(super) struct PointerState {
    active_surface_id: Option<ObjectId>,
    pub(super) drag_and_drop: DragAndDrop,
    serial: u32,
}

impl PointerDataExt for PointerUserData {
    fn pointer_data(&self) -> &PointerData {
        &self.pdata
    }
}

#[derive(Clone, Debug)]
pub struct PendingMouse {
    window_id: usize,
    pub(super) copy_and_paste: Arc<Mutex<CopyAndPaste>>,
    surface_coords: Option<(f64, f64)>,
    button: Vec<(MousePress, ButtonState)>,
    scroll: Option<(f64, f64)>,
    in_window: bool,
}

impl PendingMouse {
    pub(super) fn create(
        window_id: usize,
        copy_and_paste: &Arc<Mutex<CopyAndPaste>>,
    ) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            window_id,
            copy_and_paste: Arc::clone(copy_and_paste),
            button: vec![],
            scroll: None,
            surface_coords: None,
            in_window: false,
        }))
    }

    pub(super) fn queue(&mut self, evt: &PointerEvent) -> bool {
        match evt.kind {
            PointerEventKind::Enter { .. } => {
                self.in_window = true;
                false
            }
            PointerEventKind::Leave { .. } => {
                let changed = self.in_window;
                self.surface_coords = None;
                self.in_window = false;
                changed
            }
            PointerEventKind::Motion { .. } => {
                let changed = self.surface_coords.is_none();
                self.surface_coords.replace(evt.position);
                changed
            }
            PointerEventKind::Press { button, .. } | PointerEventKind::Release { button, .. } => {
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
                let button_state = match evt.kind {
                    PointerEventKind::Press { .. } => ButtonState::Pressed,
                    PointerEventKind::Release { .. } => ButtonState::Released,
                    _ => unreachable!(),
                };
                self.button.push((button, button_state));
                changed
            }
            PointerEventKind::Axis {
                horizontal,
                vertical,
                ..
            } => {
                let changed = self.scroll.is_none();
                let (x, y) = self.scroll.take().unwrap_or((0., 0.));
                self.scroll
                    .replace((x + horizontal.absolute, y + vertical.absolute));
                changed
            }
        }
    }

    pub(super) fn next_button(pending: &Arc<Mutex<Self>>) -> Option<(MousePress, ButtonState)> {
        let mut pending = pending.lock().unwrap();
        if pending.button.is_empty() {
            None
        } else {
            Some(pending.button.remove(0))
        }
    }

    pub(super) fn coords(pending: &Arc<Mutex<Self>>) -> Option<(f64, f64)> {
        pending.lock().unwrap().surface_coords.take()
    }

    pub(super) fn scroll(pending: &Arc<Mutex<Self>>) -> Option<(f64, f64)> {
        pending.lock().unwrap().scroll.take()
    }

    pub(super) fn in_window(pending: &Arc<Mutex<Self>>) -> bool {
        pending.lock().unwrap().in_window
    }
}

fn event_serial(event: &PointerEvent) -> Option<u32> {
    Some(match event.kind {
        PointerEventKind::Enter { serial, .. } => serial,
        PointerEventKind::Leave { serial, .. } => serial,
        PointerEventKind::Press { serial, .. } => serial,
        PointerEventKind::Release { serial, .. } => serial,
        _ => return None,
    })
}

impl WaylandState {
    fn pointer_window_frame(&mut self, pointer: &WlPointer, events: &[PointerEvent]) {
        let windows = self.windows.borrow();

        for evt in events {
            let surface = &evt.surface;
            if surface.id() == self.active_surface_id.borrow().as_ref().unwrap().clone() {
                let (x, y) = evt.position;
                let parent_surface = match evt.surface.data::<SurfaceData>() {
                    Some(data) => match data.parent_surface() {
                        Some(sd) => sd,
                        None => continue,
                    },
                    None => continue,
                };

                let wid = SurfaceUserData::from_wl(parent_surface).window_id;
                let mut inner = windows.get(&wid).unwrap().borrow_mut();

                match evt.kind {
                    PointerEventKind::Enter { .. } => {
                        inner.window_frame.click_point_moved(
                            Duration::ZERO,
                            &evt.surface.id(),
                            x,
                            y,
                        );
                    }
                    PointerEventKind::Leave { .. } => {
                        inner.window_frame.click_point_left();
                    }
                    PointerEventKind::Motion { .. } => {
                        inner.window_frame.click_point_moved(
                            Duration::ZERO,
                            &evt.surface.id(),
                            x,
                            y,
                        );
                    }
                    PointerEventKind::Press { button, serial, .. }
                    | PointerEventKind::Release { button, serial, .. } => {
                        let pressed = if matches!(evt.kind, PointerEventKind::Press { .. }) {
                            true
                        } else {
                            false
                        };
                        let click = match button {
                            0x110 => FrameClick::Normal,
                            0x111 => FrameClick::Alternate,
                            _ => continue,
                        };
                        if let Some(action) =
                            inner.window_frame.on_click(Duration::ZERO, click, pressed)
                        {
                            inner.frame_action(pointer, serial, action);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}
