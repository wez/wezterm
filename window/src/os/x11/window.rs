use super::connection::*;
use crate::WindowCallbacks;
use failure::Fallible;
use std::convert::TryInto;
use std::sync::{Arc, Mutex};

struct WindowHolder {
    window_id: xcb::xproto::Window,
    conn: Arc<Connection>,
    callbacks: Mutex<Box<WindowCallbacks>>,
}

impl Drop for WindowHolder {
    fn drop(&mut self) {
        xcb::destroy_window(self.conn.conn(), self.window_id);
    }
}

/// A Window!
#[derive(Clone)]
pub struct Window {
    window: Arc<WindowHolder>,
}

impl Window {
    /// Create a new window on the specified screen with the specified
    /// dimensions
    pub fn new_window(
        _class_name: &str,
        name: &str,
        width: usize,
        height: usize,
        callbacks: Box<WindowCallbacks>,
    ) -> Fallible<Window> {
        let conn = Connection::get().ok_or_else(|| {
            failure::err_msg(
                "new_window must be called on the gui thread after Connection::init has succeeded",
            )
        })?;

        let window = {
            let setup = conn.conn().get_setup();
            let screen = setup
                .roots()
                .nth(conn.screen_num() as usize)
                .ok_or_else(|| failure::err_msg("no screen?"))?;

            let window_id = conn.conn().generate_id();

            xcb::create_window_checked(
                conn.conn(),
                xcb::COPY_FROM_PARENT as u8,
                window_id,
                screen.root(),
                // x, y
                0,
                0,
                // width, height
                width.try_into()?,
                height.try_into()?,
                // border width
                0,
                xcb::WINDOW_CLASS_INPUT_OUTPUT as u16,
                screen.root_visual(),
                &[(
                    xcb::CW_EVENT_MASK,
                    xcb::EVENT_MASK_EXPOSURE
                        | xcb::EVENT_MASK_KEY_PRESS
                        | xcb::EVENT_MASK_BUTTON_PRESS
                        | xcb::EVENT_MASK_BUTTON_RELEASE
                        | xcb::EVENT_MASK_POINTER_MOTION
                        | xcb::EVENT_MASK_BUTTON_MOTION
                        | xcb::EVENT_MASK_KEY_RELEASE
                        | xcb::EVENT_MASK_STRUCTURE_NOTIFY,
                )],
            )
            .request_check()?;
            Arc::new(WindowHolder {
                window_id,
                conn: Arc::clone(&conn),
                callbacks: Mutex::new(callbacks),
            })
        };

        xcb::change_property(
            &*conn,
            xcb::PROP_MODE_REPLACE as u8,
            window.window_id,
            conn.atom_protocols,
            4,
            32,
            &[conn.atom_delete],
        );

        let window = Window { window };

        conn.windows
            .borrow_mut()
            .insert(window.window.window_id, window.clone());

        window.set_title(name);
        window.show();

        Ok(window)
    }

    /// Change the title for the window manager
    pub fn set_title(&self, title: &str) {
        xcb_util::icccm::set_wm_name(self.window.conn.conn(), self.window.window_id, title);
    }

    /// Display the window
    pub fn show(&self) {
        xcb::map_window(self.window.conn.conn(), self.window.window_id);
    }

    pub fn dispatch_event(&self, event: &xcb::GenericEvent) -> Fallible<()> {
        let r = event.response_type() & 0x7f;
        match r {
            xcb::EXPOSE => {
                let expose: &xcb::ExposeEvent = unsafe { xcb::cast_event(event) };
                eprintln!("EXPOSE");
                //self.expose(expose.x(), expose.y(), expose.width(), expose.height())?;
            }
            xcb::CONFIGURE_NOTIFY => {
                let cfg: &xcb::ConfigureNotifyEvent = unsafe { xcb::cast_event(event) };
                eprintln!("CONFIGURE_NOTIFY");
                /*
                let schedule = self.have_pending_resize.is_none();
                self.have_pending_resize = Some((cfg.width(), cfg.height()));
                if schedule {
                    self.host.with_window(|win| win.check_for_resize());
                }
                */
            }
            xcb::KEY_PRESS => {
                let key_press: &xcb::KeyPressEvent = unsafe { xcb::cast_event(event) };
                eprintln!("KEY_PRESS");
                /*
                let mux = Mux::get().unwrap();
                let tab = match mux.get_active_tab_for_window(self.get_mux_window_id()) {
                    Some(tab) => tab,
                    None => return Ok(()),
                };
                if let Some((code, mods)) = self.decode_key(key_press) {
                    if self.host.process_gui_shortcuts(&*tab, mods, code)? {
                        return Ok(());
                    }

                    tab.key_down(code, mods)?;
                }
                */
            }
            xcb::MOTION_NOTIFY => {
                let motion: &xcb::MotionNotifyEvent = unsafe { xcb::cast_event(event) };
                eprintln!("MOTION_NOTIFY");
                /*

                let event = MouseEvent {
                    kind: MouseEventKind::Move,
                    button: MouseButton::None,
                    x: (motion.event_x() as usize / self.cell_width) as usize,
                    y: (motion.event_y() as usize / self.cell_height) as i64,
                    modifiers: xkeysyms::modifiers_from_state(motion.state()),
                };
                self.mouse_event(event)?;
                */
            }
            xcb::BUTTON_PRESS | xcb::BUTTON_RELEASE => {
                let button_press: &xcb::ButtonPressEvent = unsafe { xcb::cast_event(event) };
                eprintln!("BUTTON_PRESS");
                /*

                let event = MouseEvent {
                    kind: match r {
                        xcb::BUTTON_PRESS => MouseEventKind::Press,
                        xcb::BUTTON_RELEASE => MouseEventKind::Release,
                        _ => unreachable!("button event mismatch"),
                    },
                    x: (button_press.event_x() as usize / self.cell_width) as usize,
                    y: (button_press.event_y() as usize / self.cell_height) as i64,
                    button: match button_press.detail() {
                        1 => MouseButton::Left,
                        2 => MouseButton::Middle,
                        3 => MouseButton::Right,
                        4 => MouseButton::WheelUp(1),
                        5 => MouseButton::WheelDown(1),
                        _ => {
                            error!("button {} is not implemented", button_press.detail());
                            return Ok(());
                        }
                    },
                    modifiers: xkeysyms::modifiers_from_state(button_press.state()),
                };

                self.mouse_event(event)?;
                */
            }
            xcb::CLIENT_MESSAGE => {
                let msg: &xcb::ClientMessageEvent = unsafe { xcb::cast_event(event) };
                eprintln!("CLIENT_MESSAGE {:?}", msg.data().data32());
                if msg.data().data32()[0] == self.window.conn.atom_delete() {
                    eprintln!("close requested");
                    if self.window.callbacks.lock().unwrap().can_close() {
                        self.close_window();
                    }
                }
            }
            _ => {
                eprintln!("unhandled: {:x}", r);
            }
        }

        Ok(())
    }

    pub fn close_window(&self) {
        self.window
            .conn
            .windows
            .borrow_mut()
            .remove(&self.window.window_id);
        xcb::destroy_window(self.window.conn.conn(), self.window.window_id);
    }
}
