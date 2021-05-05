use ::window::{KeyCode, KeyEvent, Modifiers, WindowOps};

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

impl super::TermWindow {
    pub async fn key_event_impl(&mut self, window_key: KeyEvent, context: &dyn WindowOps) -> bool {
        if !window_key.key_is_down {
            return false;
        }

        if self.config.debug_key_events {
            log::info!("key_event {:?}", window_key);
        } else {
            log::trace!("key_event {:?}", window_key);
        }

        let pane = match self.get_active_pane_or_overlay() {
            Some(pane) => pane,
            None => return false,
        };

        // The leader key is a kind of modal modifier key.
        // It is allowed to be active for up to the leader timeout duration,
        // after which it auto-deactivates.
        let (leader_active, leader_mod) = match self.leader_is_down.as_ref() {
            Some(expiry) if *expiry > std::time::Instant::now() => {
                // Currently active
                (true, Modifiers::LEADER)
            }
            Some(_) => {
                // Expired; clear out the old expiration time
                self.leader_is_down.take();
                (false, Modifiers::NONE)
            }
            _ => (false, Modifiers::NONE),
        };

        let modifiers = window_mods_to_termwiz_mods(window_key.modifiers);
        let raw_modifiers = window_mods_to_termwiz_mods(window_key.raw_modifiers);

        // If we know the underlying raw code, let's first try any mappings
        // defined for those.  By their nature, we don't know anything useful
        // about their position or meaning in code here, so we don't have
        // any built-in mappings defined with raw_codes.
        // That means that we only need check for user-defined values in
        // this block.
        if let Some(raw_code) = window_key.raw_code {
            let raw_code_key = KeyCode::RawCode(raw_code);

            if !leader_active {
                // Check to see if this key-press is the leader activating
                if let Some(duration) = self
                    .input_map
                    .is_leader(&raw_code_key, window_key.raw_modifiers)
                {
                    // Yes; record its expiration
                    self.leader_is_down
                        .replace(std::time::Instant::now() + duration);
                    return true;
                }
            }

            if let Some(assignment) = self
                .input_map
                .lookup_key(&raw_code_key, window_key.raw_modifiers | leader_mod)
            {
                self.perform_key_assignment(&pane, &assignment).await.ok();
                context.invalidate();

                if leader_active {
                    // A successful leader key-lookup cancels the leader
                    // virtual modifier state
                    self.leader_is_down.take();
                }
                return true;
            }
        }

        // We may know the decoded platform key, but prior to any composition
        // defined by the system (eg: prior to dead key expansion).
        if let Some(key) = &window_key.raw_key {
            if !leader_active {
                // Check to see if this key-press is the leader activating
                if let Some(duration) = self.input_map.is_leader(key, window_key.raw_modifiers) {
                    // Yes; record its expiration
                    self.leader_is_down
                        .replace(std::time::Instant::now() + duration);
                    return true;
                }
            }

            if let Some(assignment) = self
                .input_map
                .lookup_key(key, window_key.raw_modifiers | leader_mod)
            {
                self.perform_key_assignment(&pane, &assignment).await.ok();
                context.invalidate();

                if leader_active {
                    // A successful leader key-lookup cancels the leader
                    // virtual modifier state
                    self.leader_is_down.take();
                }
                return true;
            }

            // While the leader modifier is active, only registered
            // keybindings are recognized.
            if !leader_active {
                let config = &self.config;

                // This is a bit ugly.
                // Not all of our platforms report LEFT|RIGHT ALT; most report just ALT.
                // For those that do distinguish between them we want to respect the left vs.
                // right settings for the compose behavior.
                // Otherwise, if the event didn't include left vs. right then we want to
                // respect the generic compose behavior.
                let bypass_compose =
                    // Left ALT and they disabled compose
                    (window_key.raw_modifiers.contains(Modifiers::LEFT_ALT)
                    && !config.send_composed_key_when_left_alt_is_pressed)
                    // Right ALT and they disabled compose
                    || (window_key.raw_modifiers.contains(Modifiers::RIGHT_ALT)
                        && !config.send_composed_key_when_right_alt_is_pressed)
                    // Generic ALT and they disabled generic compose
                    || (!window_key.raw_modifiers.contains(Modifiers::RIGHT_ALT)
                        && !window_key.raw_modifiers.contains(Modifiers::LEFT_ALT)
                        && window_key.raw_modifiers.contains(Modifiers::ALT)
                        && !config.send_composed_key_when_alt_is_pressed);

                if let Key::Code(term_key) = self.win_key_code_to_termwiz_key_code(&key) {
                    if bypass_compose && pane.key_down(term_key, raw_modifiers).is_ok() {
                        if !key.is_modifier() && self.pane_state(pane.pane_id()).overlay.is_none() {
                            self.maybe_scroll_to_bottom_for_input(&pane);
                        }
                        context.set_cursor(None);
                        context.invalidate();
                        return true;
                    }
                }
            }
        }

        if !leader_active {
            // Check to see if this key-press is the leader activating
            if let Some(duration) = self
                .input_map
                .is_leader(&window_key.key, window_key.modifiers)
            {
                // Yes; record its expiration
                self.leader_is_down
                    .replace(std::time::Instant::now() + duration);
                return true;
            }
        }

        if let Some(assignment) = self
            .input_map
            .lookup_key(&window_key.key, window_key.modifiers | leader_mod)
        {
            self.perform_key_assignment(&pane, &assignment).await.ok();
            context.invalidate();
            if leader_active {
                // A successful leader key-lookup cancels the leader
                // virtual modifier state
                self.leader_is_down.take();
            }
            true
        } else if leader_active {
            if !window_key.key.is_modifier() {
                // Leader was pressed and this non-modifier keypress isn't
                // a registered key binding; swallow this event and cancel
                // the leader modifier
                self.leader_is_down.take();
            }
            true
        } else {
            let key = self.win_key_code_to_termwiz_key_code(&window_key.key);
            match key {
                Key::Code(key) => {
                    if pane.key_down(key, modifiers).is_ok() {
                        if !key.is_modifier() && self.pane_state(pane.pane_id()).overlay.is_none() {
                            self.maybe_scroll_to_bottom_for_input(&pane);
                        }
                        context.set_cursor(None);
                        context.invalidate();
                        true
                    } else {
                        false
                    }
                }
                Key::Composed(s) => {
                    if leader_active {
                        // Leader was pressed and this non-modifier keypress isn't
                        // a registered key binding; swallow this event and cancel
                        // the leader modifier.
                        self.leader_is_down.take();
                    } else {
                        pane.writer().write_all(s.as_bytes()).ok();
                        self.maybe_scroll_to_bottom_for_input(&pane);
                        context.invalidate();
                    }
                    true
                }
                Key::None => false,
            }
        }
    }

    fn win_key_code_to_termwiz_key_code(&self, key: &::window::KeyCode) -> Key {
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
