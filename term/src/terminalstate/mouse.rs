use crate::input::*;
use crate::terminalstate::MouseEncoding;
use crate::TerminalState;
use anyhow::bail;

impl TerminalState {
    /// Encode a coordinate value using X10 encoding.
    /// X10 has a theoretical maximum coordinate value of 255-33, but
    /// because we emit UTF-8 we are effectively capped at the maximum
    /// single byte character value of 127, with coordinates capping
    /// out at 127-33.
    /// This isn't "fixable" in X10 encoding, applications should
    /// use the superior SGR mouse encoding scheme instead.
    fn legacy_mouse_coord(position: i64) -> char {
        position.max(0).saturating_add(1 + 32).min(127) as u8 as char
    }

    fn mouse_report_button_number(&self, event: &MouseEvent) -> (i8, MouseButton) {
        let button = match event.button {
            MouseButton::None => self
                .current_mouse_buttons
                .last()
                .copied()
                .unwrap_or(MouseButton::None),
            b => b,
        };
        let mut code = match button {
            MouseButton::None => 3,
            MouseButton::Left => 0,
            MouseButton::Middle => 1,
            MouseButton::Right => 2,
            MouseButton::WheelUp(_) => 64,
            MouseButton::WheelDown(_) => 65,
        };

        if event.modifiers.contains(KeyModifiers::SHIFT) {
            code += 4;
        }
        if event.modifiers.contains(KeyModifiers::ALT) {
            code += 8;
        }
        if event.modifiers.contains(KeyModifiers::CTRL) {
            code += 16;
        }

        (code, button)
    }

    fn mouse_wheel(&mut self, event: MouseEvent) -> anyhow::Result<()> {
        let (button, _button) = self.mouse_report_button_number(&event);

        if self.mouse_encoding == MouseEncoding::SGR
            && (self.mouse_tracking || self.button_event_mouse || self.any_event_mouse)
        {
            write!(
                self.writer,
                "\x1b[<{};{};{}M",
                button,
                event.x + 1,
                event.y + 1
            )?;
            self.writer.flush()?;
        } else if self.mouse_encoding == MouseEncoding::SgrPixels
            && (self.mouse_tracking || self.button_event_mouse || self.any_event_mouse)
        {
            let height = self.screen.physical_rows as usize;
            let width = self.screen.physical_cols as usize;
            write!(
                self.writer,
                "\x1b[<{};{};{}M",
                button,
                (event.x * (self.pixel_width / width)) + event.x_pixel_offset + 1,
                (event.y as usize * (self.pixel_height / height)) + event.y_pixel_offset + 1
            )?;
            self.writer.flush()?;
        } else if self.mouse_tracking || self.button_event_mouse || self.any_event_mouse {
            write!(
                self.writer,
                "\x1b[M{}{}{}",
                (32 + button) as u8 as char,
                Self::legacy_mouse_coord(event.x as i64),
                Self::legacy_mouse_coord(event.y),
            )?;
            self.writer.flush()?;
        } else if self.screen.is_alt_screen_active() {
            // Send cursor keys instead (equivalent to xterm's alternateScroll mode)
            for _ in 0..self.config.alternate_buffer_wheel_scroll_speed() {
                self.key_down(
                    match event.button {
                        MouseButton::WheelDown(_) => KeyCode::DownArrow,
                        MouseButton::WheelUp(_) => KeyCode::UpArrow,
                        _ => bail!("unexpected mouse event"),
                    },
                    KeyModifiers::default(),
                )?;
            }
        }
        Ok(())
    }

    fn mouse_button_press(&mut self, event: MouseEvent) -> anyhow::Result<()> {
        let (button, event_button) = self.mouse_report_button_number(&event);
        self.current_mouse_buttons.retain(|&b| b != event_button);
        self.current_mouse_buttons.push(event_button);

        if !(self.mouse_tracking || self.button_event_mouse || self.any_event_mouse) {
            return Ok(());
        }

        if self.mouse_encoding == MouseEncoding::SGR {
            write!(
                self.writer,
                "\x1b[<{};{};{}M",
                button,
                event.x + 1,
                event.y + 1
            )?;
            self.writer.flush()?;
        } else if self.mouse_encoding == MouseEncoding::SgrPixels {
            let height = self.screen.physical_rows as usize;
            let width = self.screen.physical_cols as usize;
            write!(
                self.writer,
                "\x1b[<{};{};{}M",
                button,
                (event.x * (self.pixel_width / width)) + event.x_pixel_offset + 1,
                (event.y as usize * (self.pixel_height / height)) + event.y_pixel_offset + 1
            )?;
            self.writer.flush()?;
        } else {
            write!(
                self.writer,
                "\x1b[M{}{}{}",
                (32 + button) as u8 as char,
                Self::legacy_mouse_coord(event.x as i64),
                Self::legacy_mouse_coord(event.y),
            )?;
            self.writer.flush()?;
        }

        Ok(())
    }

    fn mouse_button_release(&mut self, event: MouseEvent) -> anyhow::Result<()> {
        let (release_button, button) = self.mouse_report_button_number(&event);
        if !self.current_mouse_buttons.is_empty() {
            self.current_mouse_buttons.retain(|&b| b != button);
            if self.mouse_tracking || self.button_event_mouse || self.any_event_mouse {
                if self.mouse_encoding == MouseEncoding::SGR {
                    write!(
                        self.writer,
                        "\x1b[<{};{};{}m",
                        release_button,
                        event.x + 1,
                        event.y + 1
                    )?;
                    self.writer.flush()?;
                } else if self.mouse_encoding == MouseEncoding::SgrPixels {
                    let height = self.screen.physical_rows as usize;
                    let width = self.screen.physical_cols as usize;
                    write!(
                        self.writer,
                        "\x1b[<{};{};{}m",
                        release_button,
                        (event.x * (self.pixel_width / width)) + event.x_pixel_offset + 1,
                        (event.y as usize * (self.pixel_height / height))
                            + event.y_pixel_offset
                            + 1
                    )?;
                    self.writer.flush()?;
                } else {
                    let release_button = 3;
                    write!(
                        self.writer,
                        "\x1b[M{}{}{}",
                        (32 + release_button) as u8 as char,
                        Self::legacy_mouse_coord(event.x as i64),
                        Self::legacy_mouse_coord(event.y),
                    )?;
                    self.writer.flush()?;
                }
            }
        }

        Ok(())
    }

    fn mouse_move(&mut self, event: MouseEvent) -> anyhow::Result<()> {
        let moved = match (&self.last_mouse_move, self.mouse_encoding) {
            (None, _) => true,
            (Some(last), MouseEncoding::SgrPixels) => {
                last.x != event.x
                    || last.y != event.y
                    || last.x_pixel_offset != event.x_pixel_offset
                    || last.y_pixel_offset != event.y_pixel_offset
            }
            (Some(last), _) => last.x != event.x || last.y != event.y,
        };

        let reportable = (self.any_event_mouse || !self.current_mouse_buttons.is_empty()) && moved;
        // Note: self.mouse_tracking on its own is for clicks, not drags!
        if reportable && (self.button_event_mouse || self.any_event_mouse) {
            match self.last_mouse_move.as_ref() {
                Some(last) if *last == event => {
                    return Ok(());
                }
                _ => {}
            }
            self.last_mouse_move.replace(event.clone());

            let (button, _button) = self.mouse_report_button_number(&event);
            let button = 32 + button;

            if self.mouse_encoding == MouseEncoding::SGR {
                write!(
                    self.writer,
                    "\x1b[<{};{};{}M",
                    button,
                    event.x + 1,
                    event.y + 1
                )?;
                self.writer.flush()?;
            } else if self.mouse_encoding == MouseEncoding::SgrPixels {
                let height = self.screen.physical_rows as usize;
                let width = self.screen.physical_cols as usize;
                write!(
                    self.writer,
                    "\x1b[<{};{};{}M",
                    button,
                    (event.x * (self.pixel_width / width)) + event.x_pixel_offset + 1,
                    (event.y as usize * (self.pixel_height / height)) + event.y_pixel_offset + 1
                )?;
                self.writer.flush()?;
            } else {
                write!(
                    self.writer,
                    "\x1b[M{}{}{}",
                    (32 + button) as u8 as char,
                    Self::legacy_mouse_coord(event.x as i64),
                    Self::legacy_mouse_coord(event.y),
                )?;
                self.writer.flush()?;
            }
        }
        Ok(())
    }

    /// Informs the terminal of a mouse event.
    /// If mouse reporting has been activated, the mouse event will be encoded
    /// appropriately and written to the associated writer.
    pub fn mouse_event(&mut self, mut event: MouseEvent) -> anyhow::Result<()> {
        // Clamp the mouse coordinates to the size of the model.
        // This situation can trigger for example when the
        // window is resized and leaves a partial row at the bottom of the
        // terminal.  The mouse can move over that portion and the gui layer
        // can thus send us out-of-bounds row or column numbers.  We want to
        // make sure that we clamp this and handle it nicely at the model layer.
        event.y = event.y.min(self.screen().physical_rows as i64 - 1);
        event.x = event.x.min(self.screen().physical_cols - 1);

        match event {
            MouseEvent {
                kind: MouseEventKind::Press,
                button: MouseButton::WheelUp(_),
                ..
            }
            | MouseEvent {
                kind: MouseEventKind::Press,
                button: MouseButton::WheelDown(_),
                ..
            } => self.mouse_wheel(event),
            MouseEvent {
                kind: MouseEventKind::Press,
                ..
            } => self.mouse_button_press(event),
            MouseEvent {
                kind: MouseEventKind::Release,
                ..
            } => self.mouse_button_release(event),
            MouseEvent {
                kind: MouseEventKind::Move,
                ..
            } => self.mouse_move(event),
        }
    }
}
