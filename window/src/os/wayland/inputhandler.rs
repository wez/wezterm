//! Implements zwp_text_input_v3 for handling IME
use std::sync::Mutex;

use smithay_client_toolkit::globals::GlobalData;
use wayland_client::{Dispatch, Proxy, QueueHandle};
use wayland_protocols::wp::text_input::zv3::client::zwp_text_input_manager_v3::ZwpTextInputManagerV3;
use wayland_protocols::wp::text_input::zv3::client::zwp_text_input_v3::ZwpTextInputV3;

use super::state::WaylandState;

pub(super) struct TextInputState {
    text_input_manager: ZwpTextInputManagerV3,
}

#[derive(Default)]
pub(super) struct TextInputData {
    inner: Mutex<TextInputDataInner>,
}

#[derive(Default)]
pub(super) struct TextInputDataInner {}

impl Dispatch<ZwpTextInputManagerV3, GlobalData, WaylandState> for TextInputState {
    fn event(
        _state: &mut WaylandState,
        _proxy: &ZwpTextInputManagerV3,
        _event: <ZwpTextInputManagerV3 as Proxy>::Event,
        _data: &GlobalData,
        _conn: &wayland_client::Connection,
        _qhandle: &QueueHandle<WaylandState>,
    ) {
        todo!()
    }
}

impl Dispatch<ZwpTextInputV3, TextInputData, WaylandState> for TextInputState {
    fn event(
        _state: &mut WaylandState,
        _proxy: &ZwpTextInputV3,
        _event: <ZwpTextInputV3 as Proxy>::Event,
        _data: &TextInputData,
        _conn: &wayland_client::Connection,
        _qhandle: &QueueHandle<WaylandState>,
    ) {
        todo!()
    }
}
