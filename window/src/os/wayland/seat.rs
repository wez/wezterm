use smithay_client_toolkit::compositor::SurfaceData;
use smithay_client_toolkit::seat::pointer::ThemeSpec;
use smithay_client_toolkit::seat::{Capability, SeatHandler, SeatState};
use wayland_client::protocol::wl_seat::WlSeat;
use wayland_client::{Connection, QueueHandle};

use crate::wayland::keyboard::KeyboardData;
use crate::wayland::pointer::PointerUserData;

use super::state::WaylandState;

impl SeatHandler for WaylandState {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat
    }

    fn new_seat(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _seat: WlSeat) {}

    fn new_capability(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        seat: WlSeat,
        capability: smithay_client_toolkit::seat::Capability,
    ) {
        if capability == Capability::Keyboard && self.keyboard.is_none() {
            log::trace!(
                "Setting keyboard
            capability"
            );
            let keyboard = seat.get_keyboard(qh, KeyboardData {});
            self.keyboard = Some(keyboard.clone());

            if let Some(text_input) = &self.text_input {
                text_input.advise_seat(&seat, &keyboard, qh);
            }
        }

        if capability == Capability::Pointer && self.pointer.is_none() {
            log::trace!(
                "Setting
            pointer capability"
            );
            let surface = self.compositor.create_surface(qh);
            let pointer = self
                .seat
                .get_pointer_with_theme_and_data::<WaylandState, SurfaceData, PointerUserData>(
                    qh,
                    &seat,
                    self.shm.wl_shm(),
                    surface,
                    ThemeSpec::System,
                    PointerUserData::new(seat.clone()),
                )
                .expect("Failed to create pointer");
            self.pointer = Some(pointer);

            let data_device_manager = &self.data_device_manager_state;
            let data_device = data_device_manager.get_data_device(qh, &seat);
            self.data_device.replace(data_device);

            let primary_select_device = self
                .primary_selection_manager
                .as_ref()
                .map(|m| m.get_selection_device(qh, &seat));
            self.primary_selection_device = primary_select_device;
        }
    }

    fn remove_capability(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _seat: WlSeat,
        _capability: smithay_client_toolkit::seat::Capability,
    ) {
        // we need to clean up the keyboard and pointer resources we created earlier
        todo!()
    }

    fn remove_seat(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _seat: WlSeat) {}
}
