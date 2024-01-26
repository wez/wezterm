use std::borrow::BorrowMut;
use std::io::Read;
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::fs::FileExt;

use wayland_client::protocol::wl_keyboard::{Event as WlKeyboardEvent, KeymapFormat, WlKeyboard};
use wayland_client::{Dispatch, Proxy};

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
                let mut file = unsafe { std::fs::File::from_raw_fd(fd.as_raw_fd()) };
                match format.into_result().unwrap() {
                    KeymapFormat::XkbV1 => {
                        let mut data = vec![0u8; *size as usize];
                        // If we weren't passed a pipe, be sure to explicitly
                        // read from the start of the file
                        match file.read_exact_at(&mut data, 0) {
                            Ok(_) => {}
                            Err(err) => {
                                // ideally: we check for:
                                // err.kind() == std::io::ErrorKind::NotSeekable
                                // but that is not yet in stable rust
                                if err.raw_os_error() == Some(libc::ESPIPE) {
                                    // It's a pipe, which cannot be seeked, so we
                                    // just try reading from the current pipe position
                                    file.read(&mut data).expect("read from Keymap fd/pipe");
                                } else {
                                    return Err(err).expect("read_exact_at from Keymap fd");
                                }
                            }
                        }
                        // Dance around CString panicing on the NUL terminator
                        // in the xkbcommon crate
                        while let Some(0) = data.last() {
                            data.pop();
                        }
                        let s = String::from_utf8(data).expect("Failed to read string from data");
                        match KeyboardWithFallback::new_from_string(s) {
                            Ok(k) => {
                                state.keyboard_mapper.replace(k);
                            }
                            Err(err) => {
                                log::error!("Error processing keymap change: {:#}", err);
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
