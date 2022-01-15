use ::window::{DeadKeyStatus, KeyCode, KeyEvent, Modifiers, RawKeyEvent, WindowOps};
use anyhow::Context;
use mux::pane::Pane;
use smol::Timer;
use std::rc::Rc;
use termwiz::input::KeyboardEncoding;

pub fn window_mods_to_termwiz_mods(modifiers: ::window::Modifiers) -> termwiz::input::Modifiers {
    let mut result = termwiz::input::Modifiers::NONE;
    if modifiers.contains(::window::Modifiers::SHIFT) {
        result.insert(termwiz::input::Modifiers::SHIFT);
    }
    if modifiers.contains(::window::Modifiers::LEFT_ALT) {
        result.insert(termwiz::input::Modifiers::ALT);
    }
    if modifiers.contains(::window::Modifiers::RIGHT_ALT) {
        result.insert(termwiz::input::Modifiers::ALT);
    }
    if modifiers.contains(::window::Modifiers::ALT) {
        result.insert(termwiz::input::Modifiers::ALT);
    }
    if modifiers.contains(::window::Modifiers::CTRL) {
        result.insert(termwiz::input::Modifiers::CTRL);
    }
    if modifiers.contains(::window::Modifiers::SUPER) {
        result.insert(termwiz::input::Modifiers::SUPER);
    }
    if modifiers.contains(::window::Modifiers::LEADER) {
        result.insert(termwiz::input::Modifiers::LEADER);
    }
    result
}

#[derive(Debug)]
pub enum Key {
    Code(::termwiz::input::KeyCode),
    Composed(String),
    None,
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
enum OnlyKeyBindings {
    Yes,
    No,
}

impl super::TermWindow {
    fn encode_win32_input(&self, pane: &Rc<dyn Pane>, key: &KeyEvent) -> Option<String> {
        if !self.config.allow_win32_input_mode
            || pane.get_keyboard_encoding() != KeyboardEncoding::Win32
        {
            return None;
        }
        key.encode_win32_input_mode()
    }

    fn process_key(
        &mut self,
        pane: &Rc<dyn Pane>,
        context: &dyn WindowOps,
        keycode: &KeyCode,
        raw_modifiers: Modifiers,
        leader_active: bool,
        leader_mod: Modifiers,
        only_key_bindings: OnlyKeyBindings,
        is_down: bool,
    ) -> bool {
        if is_down && !leader_active {
            // Check to see if this key-press is the leader activating
            if let Some(duration) = self.input_map.is_leader(&keycode, raw_modifiers) {
                // Yes; record its expiration
                let target = std::time::Instant::now() + duration;
                self.leader_is_down.replace(target);
                self.update_title();
                // schedule an invalidation so that the cursor or status
                // area will be repainted at the right time
                if let Some(window) = self.window.clone() {
                    promise::spawn::spawn(async move {
                        Timer::at(target).await;
                        window.invalidate();
                    })
                    .detach();
                }
                return true;
            }
        }

        if is_down {
            if let Some(assignment) = self
                .input_map
                .lookup_key(&keycode, raw_modifiers | leader_mod)
            {
                if self.config.debug_key_events {
                    log::info!(
                        "{:?} {:?} -> perform {:?}",
                        keycode,
                        raw_modifiers | leader_mod,
                        assignment
                    );
                }
                self.perform_key_assignment(&pane, &assignment).ok();
                context.invalidate();

                if leader_active {
                    // A successful leader key-lookup cancels the leader
                    // virtual modifier state
                    self.leader_done();
                }
                return true;
            }
        }

        // While the leader modifier is active, only registered
        // keybindings are recognized.
        let only_key_bindings = match (only_key_bindings, leader_active) {
            (OnlyKeyBindings::Yes, _) => OnlyKeyBindings::Yes,
            (_, true) => OnlyKeyBindings::Yes,
            _ => OnlyKeyBindings::No,
        };

        if only_key_bindings == OnlyKeyBindings::No {
            let config = &self.config;

            // This is a bit ugly.
            // Not all of our platforms report LEFT|RIGHT ALT; most report just ALT.
            // For those that do distinguish between them we want to respect the left vs.
            // right settings for the compose behavior.
            // Otherwise, if the event didn't include left vs. right then we want to
            // respect the generic compose behavior.
            let bypass_compose =
                    // Left ALT and they disabled compose
                    (raw_modifiers.contains(Modifiers::LEFT_ALT)
                    && !config.send_composed_key_when_left_alt_is_pressed)
                    // Right ALT and they disabled compose
                    || (raw_modifiers.contains(Modifiers::RIGHT_ALT)
                        && !config.send_composed_key_when_right_alt_is_pressed)
                    // Generic ALT and they disabled generic compose
                    || (!raw_modifiers.contains(Modifiers::RIGHT_ALT)
                        && !raw_modifiers.contains(Modifiers::LEFT_ALT)
                        && raw_modifiers.contains(Modifiers::ALT)
                        && !config.send_composed_key_when_alt_is_pressed);

            if bypass_compose {
                if let Key::Code(term_key) = self.win_key_code_to_termwiz_key_code(keycode) {
                    let tw_raw_modifiers = window_mods_to_termwiz_mods(raw_modifiers);
                    if self.config.debug_key_events {
                        log::info!(
                            "{:?} {:?} -> send to pane {:?} {:?}",
                            keycode,
                            raw_modifiers,
                            term_key,
                            tw_raw_modifiers
                        );
                    }

                    let res = if is_down {
                        pane.key_down(term_key, tw_raw_modifiers)
                    } else {
                        pane.key_up(term_key, tw_raw_modifiers)
                    };

                    if res.is_ok() {
                        if is_down
                            && !keycode.is_modifier()
                            && self.pane_state(pane.pane_id()).overlay.is_none()
                        {
                            self.maybe_scroll_to_bottom_for_input(&pane);
                        }
                        context.set_cursor(None);
                        if !keycode.is_modifier() {
                            context.invalidate();
                        }

                        return true;
                    }
                }
            }
        }

        false
    }

    pub fn raw_key_event_impl(&mut self, key: RawKeyEvent, context: &dyn WindowOps) {
        if self.config.debug_key_events {
            log::info!("key_event {:?}", key);
        } else {
            log::trace!("key_event {:?}", key);
        }

        // The leader key is a kind of modal modifier key.
        // It is allowed to be active for up to the leader timeout duration,
        // after which it auto-deactivates.
        let (leader_active, leader_mod) = if self.leader_is_active_mut() {
            // Currently active
            (true, Modifiers::LEADER)
        } else {
            (false, Modifiers::NONE)
        };

        let pane = match self.get_active_pane_or_overlay() {
            Some(pane) => pane,
            None => return,
        };

        // First, try to match raw physical key
        let phys_key = match &key.key {
            phys @ KeyCode::Physical(_) => Some(phys.clone()),
            _ => key.phys_code.map(KeyCode::Physical),
        };

        if let Some(phys_key) = &phys_key {
            if self.process_key(
                &pane,
                context,
                &phys_key,
                key.modifiers,
                leader_active,
                leader_mod,
                OnlyKeyBindings::Yes,
                key.key_is_down,
            ) {
                key.set_handled();
                return;
            }
        }

        // Then try the raw code
        let raw_key = match &key.key {
            raw @ KeyCode::RawCode(_) => raw.clone(),
            _ => KeyCode::RawCode(key.raw_code),
        };
        if self.process_key(
            &pane,
            context,
            &raw_key,
            key.modifiers,
            leader_active,
            leader_mod,
            OnlyKeyBindings::Yes,
            key.key_is_down,
        ) {
            key.set_handled();
            return;
        }

        if phys_key.as_ref() == Some(&key.key) || raw_key == key.key {
            // We already matched against whatever key.key is, so no need
            // to do it again below
            return;
        }

        if self.process_key(
            &pane,
            context,
            &key.key,
            key.modifiers,
            leader_active,
            leader_mod,
            OnlyKeyBindings::Yes,
            key.key_is_down,
        ) {
            key.set_handled();
        }
    }

    pub fn leader_is_active(&self) -> bool {
        match self.leader_is_down.as_ref() {
            Some(expiry) if *expiry > std::time::Instant::now() => {
                self.update_next_frame_time(Some(*expiry));
                true
            }
            Some(_) => false,
            None => false,
        }
    }

    pub fn leader_is_active_mut(&mut self) -> bool {
        match self.leader_is_down.as_ref() {
            Some(expiry) if *expiry > std::time::Instant::now() => {
                self.update_next_frame_time(Some(*expiry));
                true
            }
            Some(_) => {
                self.leader_done();
                false
            }
            None => false,
        }
    }

    pub fn composition_status(&self) -> &DeadKeyStatus {
        &self.dead_key_status
    }

    fn leader_done(&mut self) {
        self.leader_is_down.take();
        self.update_title();
        if let Some(window) = &self.window {
            window.invalidate();
        }
    }

    pub fn key_event_impl(&mut self, window_key: KeyEvent, context: &dyn WindowOps) {
        if self.config.debug_key_events {
            log::info!("key_event {:?}", window_key);
        } else {
            log::trace!("key_event {:?}", window_key);
        }

        let pane = match self.get_active_pane_or_overlay() {
            Some(pane) => pane,
            None => return,
        };

        // The leader key is a kind of modal modifier key.
        // It is allowed to be active for up to the leader timeout duration,
        // after which it auto-deactivates.
        let (leader_active, leader_mod) = if self.leader_is_active_mut() {
            // Currently active
            (true, Modifiers::LEADER)
        } else {
            (false, Modifiers::NONE)
        };

        let modifiers = window_mods_to_termwiz_mods(window_key.modifiers);

        if self.process_key(
            &pane,
            context,
            &window_key.key,
            window_key.modifiers,
            leader_active,
            leader_mod,
            OnlyKeyBindings::No,
            window_key.key_is_down,
        ) {
            return;
        }

        let key = self.win_key_code_to_termwiz_key_code(&window_key.key);

        match key {
            Key::Code(key) => {
                if window_key.key_is_down && leader_active && !key.is_modifier() {
                    // Leader was pressed and this non-modifier keypress isn't
                    // a registered key binding; swallow this event and cancel
                    // the leader modifier.
                    self.leader_done();
                    return;
                }

                if self.config.debug_key_events {
                    log::info!(
                        "send to pane {} key={:?} mods={:?}",
                        if window_key.key_is_down { "DOWN" } else { "UP" },
                        key,
                        modifiers
                    );
                }

                let res = if let Some(encoded) = self.encode_win32_input(&pane, &window_key) {
                    if self.config.debug_key_events {
                        log::info!("Encoded input as {:?}", encoded);
                    }
                    pane.writer()
                        .write_all(encoded.as_bytes())
                        .context("sending win32-input-mode encoded data")
                } else if window_key.key_is_down {
                    pane.key_down(key, modifiers)
                } else {
                    pane.key_up(key, modifiers)
                };

                if res.is_ok() {
                    if window_key.key_is_down
                        && !key.is_modifier()
                        && self.pane_state(pane.pane_id()).overlay.is_none()
                    {
                        self.maybe_scroll_to_bottom_for_input(&pane);
                    }
                    context.set_cursor(None);
                    if !key.is_modifier() {
                        context.invalidate();
                    }
                }
            }
            Key::Composed(s) => {
                if !window_key.key_is_down {
                    return;
                }
                if leader_active {
                    // Leader was pressed and this non-modifier keypress isn't
                    // a registered key binding; swallow this event and cancel
                    // the leader modifier.
                    self.leader_done();
                    return;
                }
                if self.config.debug_key_events {
                    log::info!("send to pane string={:?}", s);
                }
                pane.writer().write_all(s.as_bytes()).ok();
                self.maybe_scroll_to_bottom_for_input(&pane);
                context.invalidate();
            }
            Key::None => {}
        }
    }

    pub fn win_key_code_to_termwiz_key_code(&self, key: &::window::KeyCode) -> Key {
        use ::termwiz::input::KeyCode as KC;
        use ::window::KeyCode as WK;

        let code = match key {
            // TODO: consider eliminating these codes from termwiz::input::KeyCode
            WK::Char('\r') => KC::Enter,
            WK::Char('\t') => KC::Tab,
            WK::Char('\u{08}') => {
                if self.config.swap_backspace_and_delete {
                    KC::Delete
                } else {
                    KC::Backspace
                }
            }
            WK::Char('\u{7f}') => {
                if self.config.swap_backspace_and_delete {
                    KC::Backspace
                } else {
                    KC::Delete
                }
            }
            WK::Char('\u{1b}') => KC::Escape,
            WK::RawCode(_) => return Key::None,
            WK::Physical(phys) => {
                return self.win_key_code_to_termwiz_key_code(&phys.to_key_code())
            }

            WK::Char(c) => KC::Char(*c),
            WK::Composed(ref s) => {
                let mut chars = s.chars();
                if let Some(first_char) = chars.next() {
                    if chars.next().is_none() {
                        // Was just a single char after all
                        return self.win_key_code_to_termwiz_key_code(&WK::Char(first_char));
                    }
                }
                return Key::Composed(s.to_owned());
            }
            WK::Function(f) => KC::Function(*f),
            WK::LeftArrow => KC::LeftArrow,
            WK::RightArrow => KC::RightArrow,
            WK::UpArrow => KC::UpArrow,
            WK::DownArrow => KC::DownArrow,
            WK::Home => KC::Home,
            WK::End => KC::End,
            WK::PageUp => KC::PageUp,
            WK::PageDown => KC::PageDown,
            WK::Insert => KC::Insert,
            WK::Hyper => KC::Hyper,
            WK::Super => KC::Super,
            WK::Meta => KC::Meta,
            WK::Cancel => KC::Cancel,
            WK::Clear => KC::Clear,
            WK::Shift => KC::Shift,
            WK::LeftShift => KC::LeftShift,
            WK::RightShift => KC::RightShift,
            WK::Control => KC::Control,
            WK::LeftControl => KC::LeftControl,
            WK::RightControl => KC::RightControl,
            WK::Alt => KC::Alt,
            WK::LeftAlt => KC::LeftAlt,
            WK::RightAlt => KC::RightAlt,
            WK::Pause => KC::Pause,
            WK::CapsLock => KC::CapsLock,
            WK::VoidSymbol => return Key::None,
            WK::Select => KC::Select,
            WK::Print => KC::Print,
            WK::Execute => KC::Execute,
            WK::PrintScreen => KC::PrintScreen,
            WK::Help => KC::Help,
            WK::LeftWindows => KC::LeftWindows,
            WK::RightWindows => KC::RightWindows,
            WK::Sleep => KC::Sleep,
            WK::Multiply => KC::Multiply,
            WK::Applications => KC::Applications,
            WK::Add => KC::Add,
            WK::Numpad(0) => KC::Numpad0,
            WK::Numpad(1) => KC::Numpad1,
            WK::Numpad(2) => KC::Numpad2,
            WK::Numpad(3) => KC::Numpad3,
            WK::Numpad(4) => KC::Numpad4,
            WK::Numpad(5) => KC::Numpad5,
            WK::Numpad(6) => KC::Numpad6,
            WK::Numpad(7) => KC::Numpad7,
            WK::Numpad(8) => KC::Numpad8,
            WK::Numpad(9) => KC::Numpad9,
            WK::Numpad(_) => return Key::None,
            WK::Separator => KC::Separator,
            WK::Subtract => KC::Subtract,
            WK::Decimal => KC::Decimal,
            WK::Divide => KC::Divide,
            WK::NumLock => KC::NumLock,
            WK::ScrollLock => KC::ScrollLock,
            WK::BrowserBack => KC::BrowserBack,
            WK::BrowserForward => KC::BrowserForward,
            WK::BrowserRefresh => KC::BrowserRefresh,
            WK::BrowserStop => KC::BrowserStop,
            WK::BrowserSearch => KC::BrowserSearch,
            WK::BrowserFavorites => KC::BrowserFavorites,
            WK::BrowserHome => KC::BrowserHome,
            WK::VolumeMute => KC::VolumeMute,
            WK::VolumeDown => KC::VolumeDown,
            WK::VolumeUp => KC::VolumeUp,
            WK::MediaNextTrack => KC::MediaNextTrack,
            WK::MediaPrevTrack => KC::MediaPrevTrack,
            WK::MediaStop => KC::MediaStop,
            WK::MediaPlayPause => KC::MediaPlayPause,
            WK::ApplicationLeftArrow => KC::ApplicationLeftArrow,
            WK::ApplicationRightArrow => KC::ApplicationRightArrow,
            WK::ApplicationUpArrow => KC::ApplicationUpArrow,
            WK::ApplicationDownArrow => KC::ApplicationDownArrow,
        };
        Key::Code(code)
    }
}
