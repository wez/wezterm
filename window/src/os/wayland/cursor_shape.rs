use smithay_client_toolkit::globals::GlobalData;
use wayland_client::Dispatch;
use wayland_protocols::wp::cursor_shape::v1::client::wp_cursor_shape_device_v1::WpCursorShapeDeviceV1;
use wayland_protocols::wp::cursor_shape::v1::client::wp_cursor_shape_manager_v1::WpCursorShapeManagerV1;

use super::state::WaylandState;

pub(super) struct CursorShapeManagerState {}

impl Dispatch<WpCursorShapeManagerV1, GlobalData, WaylandState> for CursorShapeManagerState {
    fn event(
        state: &mut WaylandState,
        proxy: &WpCursorShapeManagerV1,
        event: <WpCursorShapeManagerV1 as wayland_client::Proxy>::Event,
        data: &GlobalData,
        conn: &wayland_client::Connection,
        qhandle: &wayland_client::QueueHandle<WaylandState>,
    ) {
        todo!()
    }
}
impl Dispatch<WpCursorShapeDeviceV1, GlobalData, WaylandState> for CursorShapeManagerState {
    fn event(
        state: &mut WaylandState,
        proxy: &WpCursorShapeDeviceV1,
        event: <WpCursorShapeDeviceV1 as wayland_client::Proxy>::Event,
        data: &GlobalData,
        conn: &wayland_client::Connection,
        qhandle: &wayland_client::QueueHandle<WaylandState>,
    ) {
        todo!()
    }
}
