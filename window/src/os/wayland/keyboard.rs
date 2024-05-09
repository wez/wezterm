use std::borrow::BorrowMut;

use wayland_client::protocol::wl_keyboard::{Event as WlKeyboardEvent, KeymapFormat, WlKeyboard};
use wayland_client::{Dispatch, Proxy};
use xkbcommon::xkb;
use xkbcommon::xkb::CONTEXT_NO_FLAGS;

use crate::x11::KeyboardWithFallback;

use super::state::WaylandState;
use super::SurfaceUserData;

// We can't use the xkbcommon feature because it is too abstract for us
impl Dispatch<WlKeyboard, KeyboardData> for WaylandState {
    fn event(
        state: &mut WaylandState,
        keyboard: &WlKeyboard,
        event: <WlKeyboard as wayland_client::Proxy>::Event,
        _data: &KeyboardData,
        _conn: &wayland_client::Connection,
        _qhandle: &wayland_client::QueueHandle<WaylandState>,
    ) {
        log::trace!("We reached an event here: {:?}???", event);
        match &event {
            WlKeyboardEvent::Enter {
                serial, surface, ..
            } => {
                *state.active_surface_id.borrow_mut() = Some(surface.id());
                *state.last_serial.borrow_mut() = *serial;
                if let Some(sud) = SurfaceUserData::try_from_wl(&surface) {
                    let window_id = sud.window_id;
                    state.keyboard_window_id.borrow_mut().replace(window_id);
                    if let Some(text_input) = &state.text_input {
                        if let Some(input) = text_input.get_text_input_for_keyboard(keyboard) {
                            input.enable();
                            input.commit();
                        }
                        text_input.advise_surface(surface, keyboard);
                    }
                } else {
                    log::warn!("{:?}, no known surface", event);
                }
            }
            WlKeyboardEvent::Leave { serial, .. } => {
                *state.last_serial.borrow_mut() = *serial;
                if let Some(text_input) = &state.text_input {
                    if let Some(input) = text_input.get_text_input_for_keyboard(keyboard) {
                        input.disable();
                        input.commit();
                    }
                }
            }
            WlKeyboardEvent::Key { serial, .. } | WlKeyboardEvent::Modifiers { serial, .. } => {
                *state.last_serial.borrow_mut() = *serial;
            }
            WlKeyboardEvent::RepeatInfo { rate, delay } => {
                *state.key_repeat_rate.borrow_mut() = *rate;
                *state.key_repeat_delay.borrow_mut() = *delay;
            }
            WlKeyboardEvent::Keymap { format, fd, size } => {
                match format.into_result().unwrap() {
                    KeymapFormat::XkbV1 => {
                        // In later protocol versions, the fd must be privately mmap'd.
                        // We let xkb handle this and then turn it back into a string.
                        #[allow(unused_unsafe)] // Upstream release will change this
                        match unsafe {
                            let context = xkb::Context::new(CONTEXT_NO_FLAGS);
                            let cloned_fd = fd.try_clone().expect("Couldn't clone owned fd");
                            xkb::Keymap::new_from_fd(
                                &context,
                                cloned_fd,
                                *size as usize,
                                xkb::KEYMAP_FORMAT_TEXT_V1,
                                xkb::COMPILE_NO_FLAGS,
                            )
                        } {
                            Ok(Some(keymap)) => {
                                let s = keymap.get_as_string(xkb::KEYMAP_FORMAT_TEXT_V1);
                                match KeyboardWithFallback::new_from_string(s) {
                                    Ok(k) => {
                                        state.keyboard_mapper.replace(k);
                                    }
                                    Err(err) => {
                                        log::error!("Error processing keymap change: {:#}", err);
                                    }
                                }
                            }
                            Ok(None) => {
                                log::error!("invalid keymap");
                            }

                            Err(err) => {
                                log::error!("{}", err);
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ => {
                unimplemented!()
            }
        }

        let Some(&window_id) = state.keyboard_window_id.as_ref() else {
            return;
        };
        let Some(win) = state.window_by_id(window_id) else {
            return;
        };
        let mut inner = win.as_ref().borrow_mut();
        let mapper = state.keyboard_mapper.borrow_mut();
        let mapper = mapper.as_mut().expect("no keymap");
        inner.keyboard_event(mapper, event);
    }
}

pub(super) struct KeyboardData {}
