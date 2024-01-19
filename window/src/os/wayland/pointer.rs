use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use smithay_client_toolkit::seat::pointer::{PointerEvent, PointerEventKind, PointerHandler};
use wayland_client::protocol::wl_pointer::{ButtonState, WlPointer};
use wayland_client::{Connection, Proxy, QueueHandle};
use wezterm_input_types::MousePress;

use super::state::WaylandState;

impl PointerHandler for WaylandState {
    fn pointer_frame(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        pointer: &WlPointer,
        events: &[PointerEvent],
    ) {
        for evt in events {
            if let PointerEventKind::Enter { .. } = &evt.kind {
                let surface_id = evt.surface.id();
                self.active_surface_id = RefCell::new(Some(surface_id));
            }
            if let Some(serial) = event_serial(&evt) {}
        }
    }
}

// pub(super) struct PointerData {
//     active_surface_id: u32,
//     surface_to_pending: HashMap<u32, Arc<Mutex<PendingMouse>>>,
//     // TODO: drag_and_drop: DragAndDrop,
//     serial: u32,
// }
//
// #[derive(Clone, Debug)]
// pub struct PendingMouse {
//     window_id: usize,
//     // TODO: copy_and_paste: Arc<Mutex<CopyAndPaste>>,
//     surface_coords: Option<(f64, f64)>,
//     button: Vec<(MousePress, ButtonState)>,
//     scroll: Option<(f64, f64)>,
//     in_window: bool,
// }

fn event_serial(event: &PointerEvent) -> Option<u32> {
    Some(match event.kind {
        PointerEventKind::Enter { serial, .. } => serial,
        PointerEventKind::Leave { serial, .. } => serial,
        PointerEventKind::Press { serial, .. } => serial,
        PointerEventKind::Release { serial, .. } => serial,
        _ => return None,
    })
}
