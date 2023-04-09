use crate::os::xkeysyms::keysym_to_keycode;
use crate::{
    DeadKeyStatus, Handled, KeyCode, KeyEvent, Modifiers, RawKeyEvent, WindowEvent,
    WindowEventSender, WindowKeyEvent,
};
use anyhow::{anyhow, ensure};
use libc;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{CStr, OsStr};
use std::os::unix::ffi::OsStrExt;
use wezterm_input_types::PhysKeyCode;
use xkb::compose::Status as ComposeStatus;
use xkbcommon::xkb;

pub struct Keyboard {
    context: xkb::Context,
    keymap: RefCell<xkb::Keymap>,
    device_id: i32,

    state: RefCell<xkb::State>,
    compose_state: RefCell<Compose>,
    phys_code_map: RefCell<HashMap<xkb::Keycode, PhysKeyCode>>,
}

struct Compose {
    state: xkb::compose::State,
    composition: String,
}

enum FeedResult {
    Composing(String),
    Composed(String, xkb::Keysym),
    Nothing(String, xkb::Keysym),
    Cancelled,
}

impl Compose {
    fn reset(&mut self) {
        self.composition.clear();
        self.state.reset();
    }

    fn feed(
        &mut self,
        xcode: xkb::Keycode,
        xsym: xkb::Keysym,
        key_state: &RefCell<xkb::State>,
    ) -> FeedResult {
        if matches!(
            self.state.status(),
            ComposeStatus::Nothing | ComposeStatus::Cancelled | ComposeStatus::Composed
        ) {
            self.composition.clear();
        }

        let previously_composing = !self.composition.is_empty();
        self.state.feed(xsym);

        match self.state.status() {
            ComposeStatus::Composing => {
                if !previously_composing {
                    // The common case for dead keys is a single combining sequence,
                    // and usually pressing the key a second time (or following it
                    // by a space) will output the key from the keycap.
                    // During composition we want to show that as the composition
                    // status, so we clock the state machine forwards to produce it,
                    // then reset and feed in the symbol again to get it ready
                    // for the next keypress

                    self.state.feed(xsym);
                    if self.state.status() == ComposeStatus::Composed {
                        if let Some(s) = self.state.utf8() {
                            self.composition = s;
                        }
                    }

                    self.state.reset();
                    self.state.feed(xsym);
                }

                if self.composition.is_empty() || previously_composing {
                    // If we didn't manage to resolve a string above,
                    // or if we're in a multi-key composition sequence,
                    // we don't have a fantastic way to indicate what is
                    // currently being composed, so we try to get something
                    // that might be meaningful by getting the utf8 for that
                    // key if known, or falling back to the name of the keysym.
                    // The keysym name is likely much wider than the utf8, but
                    // it's probably better than nothing.
                    // An alternative we could use if folks don't like it is
                    // either a space or an underscore.
                    let key_state = key_state.borrow();
                    let utf8 = key_state.key_get_utf8(xcode);
                    if !utf8.is_empty() {
                        self.composition.push_str(&utf8);
                    } else {
                        self.composition.push_str(&xkb::keysym_get_name(xsym));
                    }
                }
                FeedResult::Composing(self.composition.clone())
            }
            ComposeStatus::Composed => {
                let res = self.state.keysym();
                let composed = self.state.utf8().unwrap_or_default();
                self.state.reset();
                FeedResult::Composed(composed, res.unwrap_or(xsym))
            }
            ComposeStatus::Nothing => {
                let utf8 = key_state.borrow().key_get_utf8(xcode);
                // CTRL-<ALPHA> is helpfully encoded in the form that we would
                // send to the terminal, however, we do want the chance to
                // distinguish between eg: CTRL-i and Tab, so if we ended up
                // with a control code representation from the xkeyboard layer,
                // discard it.
                // <https://github.com/wez/wezterm/issues/1851>
                if utf8.len() == 1 && utf8.as_bytes()[0] < 0x20 {
                    FeedResult::Nothing(String::new(), xsym)
                } else {
                    FeedResult::Nothing(utf8, xsym)
                }
            }
            ComposeStatus::Cancelled => {
                self.state.reset();
                FeedResult::Cancelled
            }
        }
    }
}

impl Keyboard {
    pub fn new_from_string(s: String) -> anyhow::Result<Self> {
        let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        let keymap = xkb::Keymap::new_from_string(
            &context,
            s,
            xkbcommon::xkb::KEYMAP_FORMAT_TEXT_V1,
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        )
        .ok_or_else(|| anyhow!("Failed to parse keymap state from file"))?;

        let state = xkb::State::new(&keymap);
        let locale = query_lc_ctype()?;

        let table =
            xkb::compose::Table::new_from_locale(&context, locale, xkb::compose::COMPILE_NO_FLAGS)
                .map_err(|_| anyhow!("Failed to acquire compose table from locale"))?;
        let compose_state = xkb::compose::State::new(&table, xkb::compose::STATE_NO_FLAGS);

        let phys_code_map = build_physkeycode_map(&keymap);

        Ok(Self {
            context,
            device_id: -1,
            keymap: RefCell::new(keymap),
            state: RefCell::new(state),
            compose_state: RefCell::new(Compose {
                state: compose_state,
                composition: String::new(),
            }),
            phys_code_map: RefCell::new(phys_code_map),
        })
    }

    pub fn new(connection: &xcb::Connection) -> anyhow::Result<(Keyboard, u8)> {
        let first_ev = xcb::xkb::get_extension_data(connection)
            .ok_or_else(|| anyhow!("could not get xkb extension data"))?
            .first_event;

        let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        let device_id = xkb::x11::get_core_keyboard_device_id(&connection);
        ensure!(device_id != -1, "Couldn't find core keyboard device");

        let keymap = xkb::x11::keymap_new_from_device(
            &context,
            &connection,
            device_id,
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        );

        let state = xkb::x11::state_new_from_device(&keymap, connection, device_id);

        let locale = query_lc_ctype()?;

        let table =
            xkb::compose::Table::new_from_locale(&context, locale, xkb::compose::COMPILE_NO_FLAGS)
                .map_err(|_| anyhow!("Failed to acquire compose table from locale"))?;
        let compose_state = xkb::compose::State::new(&table, xkb::compose::STATE_NO_FLAGS);

        {
            let map_parts = xcb::xkb::MapPart::KEY_TYPES
                | xcb::xkb::MapPart::KEY_SYMS
                | xcb::xkb::MapPart::MODIFIER_MAP
                | xcb::xkb::MapPart::EXPLICIT_COMPONENTS
                | xcb::xkb::MapPart::KEY_ACTIONS
                | xcb::xkb::MapPart::KEY_BEHAVIORS
                | xcb::xkb::MapPart::VIRTUAL_MODS
                | xcb::xkb::MapPart::VIRTUAL_MOD_MAP;

            let events = xcb::xkb::EventType::NEW_KEYBOARD_NOTIFY
                | xcb::xkb::EventType::MAP_NOTIFY
                | xcb::xkb::EventType::STATE_NOTIFY;

            connection.check_request(connection.send_request_checked(&xcb::xkb::SelectEvents {
                device_spec: device_id as u16,
                affect_which: events,
                clear: xcb::xkb::EventType::empty(),
                select_all: events,
                affect_map: map_parts,
                map: map_parts,
                details: &[],
            }))?;
        }

        let phys_code_map = build_physkeycode_map(&keymap);
        let kbd = Keyboard {
            context,
            device_id,
            keymap: RefCell::new(keymap),
            state: RefCell::new(state),
            compose_state: RefCell::new(Compose {
                state: compose_state,
                composition: String::new(),
            }),
            phys_code_map: RefCell::new(phys_code_map),
        };

        Ok((kbd, first_ev))
    }

    pub fn wayland_key_repeats(&self, code: u32) -> bool {
        self.keymap.borrow().key_repeats(code + 8)
    }

    pub fn process_wayland_key(
        &self,
        code: u32,
        pressed: bool,
        events: &mut WindowEventSender,
    ) -> Option<WindowKeyEvent> {
        let want_repeat = self.wayland_key_repeats(code);
        self.process_key_event_impl(code + 8, pressed, events, want_repeat)
    }

    pub fn process_key_press_event(
        &self,
        xcb_ev: &xcb::x::KeyPressEvent,
        events: &mut WindowEventSender,
    ) {
        let xcode = xkb::Keycode::from(xcb_ev.detail());
        self.process_key_event_impl(xcode, true, events, false);
    }

    pub fn process_key_release_event(
        &self,
        xcb_ev: &xcb::x::KeyReleaseEvent,
        events: &mut WindowEventSender,
    ) {
        let xcode = xkb::Keycode::from(xcb_ev.detail());
        self.process_key_event_impl(xcode, false, events, false);
    }

    fn process_key_event_impl(
        &self,
        xcode: xkb::Keycode,
        pressed: bool,
        events: &mut WindowEventSender,
        want_repeat: bool,
    ) -> Option<WindowKeyEvent> {
        let phys_code = self.phys_code_map.borrow().get(&xcode).copied();
        let raw_modifiers = self.get_key_modifiers();

        let xsym = self.state.borrow().key_get_one_sym(xcode);
        let handled = Handled::new();

        let raw_key_event = RawKeyEvent {
            key: match phys_code {
                Some(phys) => KeyCode::Physical(phys),
                None => KeyCode::RawCode(xcode),
            },
            phys_code,
            raw_code: xcode,
            modifiers: raw_modifiers,
            repeat_count: 1,
            key_is_down: pressed,
            handled: handled.clone(),
        };

        let mut kc = None;
        let ksym = if pressed {
            events.dispatch(WindowEvent::RawKeyEvent(raw_key_event.clone()));
            if handled.is_handled() {
                self.compose_state.borrow_mut().reset();
                log::trace!("process_key_event: raw key was handled; not processing further");

                if want_repeat {
                    return Some(WindowKeyEvent::RawKeyEvent(raw_key_event));
                }
                return None;
            }

            match self
                .compose_state
                .borrow_mut()
                .feed(xcode, xsym, &self.state)
            {
                FeedResult::Composing(composition) => {
                    log::trace!(
                        "process_key_event: RawKeyEvent FeedResult::Composing: {:?}",
                        composition
                    );
                    events.dispatch(WindowEvent::AdviseDeadKeyStatus(DeadKeyStatus::Composing(
                        composition,
                    )));
                    return None;
                }
                FeedResult::Composed(utf8, sym) => {
                    if !utf8.is_empty() {
                        kc.replace(crate::KeyCode::composed(&utf8));
                    }
                    log::trace!(
                        "process_key_event: RawKeyEvent FeedResult::Composed: \
                                {:?}, {:?}. kc -> {:?}",
                        utf8,
                        sym,
                        kc
                    );
                    events.dispatch(WindowEvent::AdviseDeadKeyStatus(DeadKeyStatus::None));
                    sym
                }
                FeedResult::Nothing(utf8, sym) => {
                    if !utf8.is_empty() {
                        kc.replace(crate::KeyCode::composed(&utf8));
                    }
                    log::trace!(
                        "process_key_event: RawKeyEvent FeedResult::Nothing: \
                                {:?}, {:?}. kc -> {:?}",
                        utf8,
                        sym,
                        kc
                    );
                    sym
                }
                FeedResult::Cancelled => {
                    log::trace!("process_key_event: RawKeyEvent FeedResult::Cancelled");
                    events.dispatch(WindowEvent::AdviseDeadKeyStatus(DeadKeyStatus::None));
                    return None;
                }
            }
        } else {
            xsym
        };

        let kc = match kc {
            Some(kc) => kc,
            None => match keysym_to_keycode(ksym).or_else(|| keysym_to_keycode(xsym)) {
                Some(kc) => kc,
                None => {
                    log::trace!("keysym_to_keycode for {:?} and {:?} -> None", ksym, xsym);
                    return None;
                }
            },
        };

        let event = KeyEvent {
            key: kc,
            modifiers: raw_modifiers,
            repeat_count: 1,
            key_is_down: pressed,
            raw: Some(raw_key_event),
        }
        .normalize_shift()
        .resurface_positional_modifier_key();

        if pressed && want_repeat {
            events.dispatch(WindowEvent::KeyEvent(event.clone()));
            // Returns the event that should be repeated later
            Some(WindowKeyEvent::KeyEvent(event))
        } else {
            events.dispatch(WindowEvent::KeyEvent(event));
            None
        }
    }

    fn mod_is_active(&self, modifier: &str) -> bool {
        // [TODO] consider state  Depressed & consumed mods
        self.state
            .borrow()
            .mod_name_is_active(modifier, xkb::STATE_MODS_EFFECTIVE)
    }
    fn led_is_active(&self, led: &str) -> bool {
        self.state.borrow().led_name_is_active(led)
    }

    pub fn get_key_modifiers(&self) -> Modifiers {
        let mut res = Modifiers::default();

        if self.mod_is_active(xkb::MOD_NAME_SHIFT) {
            res |= Modifiers::SHIFT;
        }
        if self.mod_is_active(xkb::MOD_NAME_CTRL) {
            res |= Modifiers::CTRL;
        }
        if self.mod_is_active(xkb::MOD_NAME_ALT) {
            // Mod1
            res |= Modifiers::ALT;
        }
        if self.mod_is_active(xkb::MOD_NAME_LOGO) {
            // Mod4
            res |= Modifiers::SUPER;
        }
        if self.led_is_active(xkb::LED_NAME_NUM) {
            res |= Modifiers::NUM_LOCK;
        }
        if self.led_is_active(xkb::LED_NAME_CAPS) {
            res |= Modifiers::CAPS_LOCK;
        }
        res
    }

    pub fn process_xkb_event(
        &self,
        connection: &xcb::Connection,
        event: &xcb::Event,
    ) -> anyhow::Result<()> {
        match event {
            xcb::Event::Xkb(xcb::xkb::Event::StateNotify(e)) => {
                self.update_state(e);
            }
            xcb::Event::Xkb(
                xcb::xkb::Event::MapNotify(_) | xcb::xkb::Event::NewKeyboardNotify(_),
            ) => {
                self.update_keymap(connection)?;
            }
            _ => {}
        }
        Ok(())
    }

    pub fn update_modifier_state(
        &self,
        mods_depressed: u32,
        mods_latched: u32,
        mods_locked: u32,
        group: u32,
    ) {
        self.state.borrow_mut().update_mask(
            xkb::ModMask::from(mods_depressed),
            xkb::ModMask::from(mods_latched),
            xkb::ModMask::from(mods_locked),
            0,
            0,
            xkb::LayoutIndex::from(group),
        );
    }

    pub fn update_state(&self, ev: &xcb::xkb::StateNotifyEvent) {
        self.state.borrow_mut().update_mask(
            xkb::ModMask::from(ev.base_mods().bits()),
            xkb::ModMask::from(ev.latched_mods().bits()),
            xkb::ModMask::from(ev.locked_mods().bits()),
            ev.base_group() as xkb::LayoutIndex,
            ev.latched_group() as xkb::LayoutIndex,
            xkb::LayoutIndex::from(ev.locked_group() as u32),
        );
    }

    pub fn update_keymap(&self, connection: &xcb::Connection) -> anyhow::Result<()> {
        let new_keymap = xkb::x11::keymap_new_from_device(
            &self.context,
            &connection,
            self.get_device_id(),
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        );
        ensure!(
            !new_keymap.get_raw_ptr().is_null(),
            "problem with new keymap"
        );

        let new_state = xkb::x11::state_new_from_device(&new_keymap, connection, self.device_id);
        ensure!(!new_state.get_raw_ptr().is_null(), "problem with new state");
        let phys_code_map = build_physkeycode_map(&new_keymap);

        self.state.replace(new_state);
        self.keymap.replace(new_keymap);
        self.phys_code_map.replace(phys_code_map);
        Ok(())
    }

    pub fn get_device_id(&self) -> i32 {
        self.device_id
    }
}

fn query_lc_ctype() -> anyhow::Result<&'static OsStr> {
    let ptr = unsafe { libc::setlocale(libc::LC_CTYPE, std::ptr::null()) };
    ensure!(!ptr.is_null(), "failed to query locale");
    let cstr = unsafe { CStr::from_ptr(ptr) };
    Ok(OsStr::from_bytes(cstr.to_bytes()))
}

fn build_physkeycode_map(keymap: &xkb::Keymap) -> HashMap<xkb::Keycode, PhysKeyCode> {
    let mut map = HashMap::new();

    // See <https://abaines.me.uk/updates/linux-x11-keys> for info on
    // these names and how they relate to the ANSI standard US keyboard
    // See also </usr/share/X11/xkb/keycodes/evdev> on a Linux system
    // to examine the mapping. FreeBSD and other unixes will use a different
    // set of keycode values.
    // We're using the symbolic names here to look up the keycodes that
    // correspond to the various key locations.
    for (name, phys) in &[
        ("ESC", PhysKeyCode::Escape),
        ("FK01", PhysKeyCode::F1),
        ("FK02", PhysKeyCode::F2),
        ("FK03", PhysKeyCode::F3),
        ("FK04", PhysKeyCode::F4),
        ("FK05", PhysKeyCode::F5),
        ("FK06", PhysKeyCode::F6),
        ("FK07", PhysKeyCode::F7),
        ("FK08", PhysKeyCode::F8),
        ("FK09", PhysKeyCode::F9),
        ("FK10", PhysKeyCode::F10),
        ("FK11", PhysKeyCode::F11),
        ("FK12", PhysKeyCode::F12),
        // ("PRSC", Print Screen),
        // ("SCLK", Scroll Lock),
        // ("PAUS", Pause),
        ("TLDE", PhysKeyCode::Grave),
        ("AE01", PhysKeyCode::K1),
        ("AE02", PhysKeyCode::K2),
        ("AE03", PhysKeyCode::K3),
        ("AE04", PhysKeyCode::K4),
        ("AE05", PhysKeyCode::K5),
        ("AE06", PhysKeyCode::K6),
        ("AE07", PhysKeyCode::K7),
        ("AE08", PhysKeyCode::K8),
        ("AE09", PhysKeyCode::K9),
        ("AE10", PhysKeyCode::K0),
        ("AE11", PhysKeyCode::Minus),
        ("AE12", PhysKeyCode::Equal),
        ("BKSL", PhysKeyCode::Backslash),
        ("BKSP", PhysKeyCode::Backspace),
        ("INS", PhysKeyCode::Insert),
        ("HOME", PhysKeyCode::Home),
        ("PGUP", PhysKeyCode::PageUp),
        ("NMLK", PhysKeyCode::NumLock),
        ("KPDV", PhysKeyCode::KeypadDivide),
        ("KPMU", PhysKeyCode::KeypadMultiply),
        ("KPSU", PhysKeyCode::KeypadSubtract),
        ("TAB", PhysKeyCode::Tab),
        ("AD01", PhysKeyCode::Q),
        ("AD02", PhysKeyCode::W),
        ("AD03", PhysKeyCode::E),
        ("AD04", PhysKeyCode::R),
        ("AD05", PhysKeyCode::T),
        ("AD06", PhysKeyCode::Y),
        ("AD07", PhysKeyCode::U),
        ("AD08", PhysKeyCode::I),
        ("AD09", PhysKeyCode::O),
        ("AD10", PhysKeyCode::P),
        ("AD11", PhysKeyCode::LeftBracket),
        ("AD12", PhysKeyCode::RightBracket),
        ("DELE", PhysKeyCode::Delete),
        ("END", PhysKeyCode::End),
        ("PGDN", PhysKeyCode::PageDown),
        ("KP7", PhysKeyCode::Keypad7),
        ("KP8", PhysKeyCode::Keypad8),
        ("KP9", PhysKeyCode::Keypad9),
        ("KPAD", PhysKeyCode::KeypadAdd),
        ("CAPS", PhysKeyCode::CapsLock),
        ("AC01", PhysKeyCode::A),
        ("AC02", PhysKeyCode::S),
        ("AC03", PhysKeyCode::D),
        ("AC04", PhysKeyCode::F),
        ("AC05", PhysKeyCode::G),
        ("AC06", PhysKeyCode::H),
        ("AC07", PhysKeyCode::J),
        ("AC08", PhysKeyCode::K),
        ("AC09", PhysKeyCode::L),
        ("AC10", PhysKeyCode::Semicolon),
        ("AC11", PhysKeyCode::Quote),
        ("RTRN", PhysKeyCode::Return),
        ("KP4", PhysKeyCode::Keypad4),
        ("KP5", PhysKeyCode::Keypad5),
        ("KP6", PhysKeyCode::Keypad6),
        ("LFSH", PhysKeyCode::LeftShift),
        ("AB01", PhysKeyCode::Z),
        ("AB02", PhysKeyCode::X),
        ("AB03", PhysKeyCode::C),
        ("AB04", PhysKeyCode::V),
        ("AB05", PhysKeyCode::B),
        ("AB06", PhysKeyCode::N),
        ("AB07", PhysKeyCode::M),
        ("AB08", PhysKeyCode::Comma),
        ("AB09", PhysKeyCode::Period),
        ("AB10", PhysKeyCode::Slash),
        ("RTSH", PhysKeyCode::RightShift),
        ("UP", PhysKeyCode::UpArrow),
        ("KP1", PhysKeyCode::Keypad1),
        ("KP2", PhysKeyCode::Keypad2),
        ("KP3", PhysKeyCode::Keypad3),
        ("KPEN", PhysKeyCode::KeypadEnter),
        ("LCTL", PhysKeyCode::LeftControl),
        ("LALT", PhysKeyCode::LeftAlt),
        ("SPCE", PhysKeyCode::Space),
        ("RALT", PhysKeyCode::RightAlt),
        ("RCTL", PhysKeyCode::RightControl),
        ("LEFT", PhysKeyCode::LeftArrow),
        ("DOWN", PhysKeyCode::DownArrow),
        ("RGHT", PhysKeyCode::RightArrow),
        ("KP0", PhysKeyCode::Keypad0),
        ("KPDL", PhysKeyCode::KeypadDelete),
        ("LWIN", PhysKeyCode::LeftWindows),
        ("RWIN", PhysKeyCode::RightWindows),
        ("MUTE", PhysKeyCode::VolumeMute),
        ("VOL-", PhysKeyCode::VolumeDown),
        ("VOL+", PhysKeyCode::VolumeUp),
        ("HELP", PhysKeyCode::Help),
    ] {
        if let Some(code) = keymap.key_by_name(name) {
            map.insert(code, *phys);
        }
    }

    map
}
