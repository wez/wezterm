// The range_plus_one lint can't see when the LHS is not compatible with
// and inclusive range
#![cfg_attr(feature = "cargo-clippy", allow(clippy::range_plus_one))]
use super::*;
use crate::color::ColorPalette;
use anyhow::bail;
use image::{self, GenericImageView};
use log::{debug, error};
use ordered_float::NotNan;
use std::fmt::Write;
use std::sync::Arc;
use termwiz::escape::csi::{
    Cursor, CursorStyle, DecPrivateMode, DecPrivateModeCode, Device, Edit, EraseInDisplay,
    EraseInLine, Mode, Sgr, TerminalMode, TerminalModeCode, Window,
};
use termwiz::escape::osc::{ChangeColorPair, ColorOrQuery, ITermFileData, ITermProprietary};
use termwiz::escape::{Action, ControlCode, Esc, EscCode, OneBased, OperatingSystemCommand, CSI};
use termwiz::image::{ImageCell, ImageData, TextureCoordinate};
use termwiz::surface::CursorShape;
use url::Url;

struct TabStop {
    tabs: Vec<bool>,
    tab_width: usize,
}

impl TabStop {
    fn new(screen_width: usize, tab_width: usize) -> Self {
        let mut tabs = Vec::with_capacity(screen_width);

        for i in 0..screen_width {
            tabs.push((i % tab_width) == 0);
        }
        Self { tabs, tab_width }
    }

    fn set_tab_stop(&mut self, col: usize) {
        self.tabs[col] = true;
    }

    fn find_next_tab_stop(&self, col: usize) -> Option<usize> {
        for i in col + 1..self.tabs.len() {
            if self.tabs[i] {
                return Some(i);
            }
        }
        None
    }

    /// Respond to the terminal resizing.
    /// If the screen got bigger, we need to expand the tab stops
    /// into the new columns with the appropriate width.
    fn resize(&mut self, screen_width: usize) {
        let current = self.tabs.len();
        if screen_width > current {
            for i in current..screen_width {
                self.tabs.push((i % self.tab_width) == 0);
            }
        }
    }
}

#[derive(Debug, Copy, Clone)]
struct SavedCursor {
    position: CursorPosition,
    wrap_next: bool,
    insert: bool,
}

struct ScreenOrAlt {
    /// The primary screen + scrollback
    screen: Screen,
    /// The alternate screen; no scrollback
    alt_screen: Screen,
    /// Tells us which screen is active
    alt_screen_is_active: bool,
    saved_cursor: Option<SavedCursor>,
    alt_saved_cursor: Option<SavedCursor>,
}

impl Deref for ScreenOrAlt {
    type Target = Screen;

    fn deref(&self) -> &Screen {
        if self.alt_screen_is_active {
            &self.alt_screen
        } else {
            &self.screen
        }
    }
}

impl DerefMut for ScreenOrAlt {
    fn deref_mut(&mut self) -> &mut Screen {
        if self.alt_screen_is_active {
            &mut self.alt_screen
        } else {
            &mut self.screen
        }
    }
}

impl ScreenOrAlt {
    pub fn new(
        physical_rows: usize,
        physical_cols: usize,
        config: &Arc<dyn TerminalConfiguration>,
    ) -> Self {
        let screen = Screen::new(physical_rows, physical_cols, config, true);
        let alt_screen = Screen::new(physical_rows, physical_cols, config, false);

        Self {
            screen,
            alt_screen,
            alt_screen_is_active: false,
            saved_cursor: None,
            alt_saved_cursor: None,
        }
    }

    pub fn resize(&mut self, physical_rows: usize, physical_cols: usize) {
        self.screen.resize(physical_rows, physical_cols);
        self.alt_screen.resize(physical_rows, physical_cols);
    }

    pub fn activate_alt_screen(&mut self) {
        self.alt_screen_is_active = true;
        self.dirty_top_phys_rows();
    }

    pub fn activate_primary_screen(&mut self) {
        self.alt_screen_is_active = false;
        self.dirty_top_phys_rows();
    }

    // When switching between alt and primary screen, we implicitly change
    // the content associated with StableRowIndex 0..num_rows.  The muxer
    // use case needs to know to invalidate its cache, so we mark those rows
    // as dirty.
    fn dirty_top_phys_rows(&mut self) {
        let num_rows = self.screen.physical_rows;
        for line_idx in 0..num_rows {
            self.screen.line_mut(line_idx).set_dirty();
        }
    }

    pub fn is_alt_screen_active(&self) -> bool {
        self.alt_screen_is_active
    }

    pub fn saved_cursor(&mut self) -> &mut Option<SavedCursor> {
        if self.alt_screen_is_active {
            &mut self.alt_saved_cursor
        } else {
            &mut self.saved_cursor
        }
    }
}

pub struct TerminalState {
    config: Arc<dyn TerminalConfiguration>,

    screen: ScreenOrAlt,
    /// The current set of attributes in effect for the next
    /// attempt to print to the display
    pen: CellAttributes,
    /// The current cursor position, relative to the top left
    /// of the screen.  0-based index.
    cursor: CursorPosition,

    /// if true, implicitly move to the next line on the next
    /// printed character
    wrap_next: bool,

    /// If true, writing a character inserts a new cell
    insert: bool,

    /// The scroll region
    scroll_region: Range<VisibleRowIndex>,

    /// When set, modifies the sequence of bytes sent for keys
    /// designated as cursor keys.  This includes various navigation
    /// keys.  The code in key_down() is responsible for interpreting this.
    application_cursor_keys: bool,

    /// When set, modifies the sequence of bytes sent for keys
    /// in the numeric keypad portion of the keyboard.
    application_keypad: bool,

    /// When set, pasting the clipboard should bracket the data with
    /// designated marker characters.
    bracketed_paste: bool,

    /// SGR style mouse tracking and reporting is enabled
    sgr_mouse: bool,
    button_event_mouse: bool,
    current_mouse_button: MouseButton,
    cursor_visible: bool,
    dec_line_drawing_mode: bool,

    tabs: TabStop,

    /// The terminal title string
    title: String,
    palette: Option<ColorPalette>,

    pixel_width: usize,
    pixel_height: usize,

    clipboard: Option<Arc<dyn Clipboard>>,

    current_dir: Option<String>,
}

fn encode_modifiers(mods: KeyModifiers) -> u8 {
    let mut number = 0;
    if mods.contains(KeyModifiers::SHIFT) {
        number |= 1;
    }
    if mods.contains(KeyModifiers::ALT) {
        number |= 2;
    }
    if mods.contains(KeyModifiers::CTRL) {
        number |= 4;
    }
    number
}

fn csi_u_encode(buf: &mut String, c: char, mods: KeyModifiers) -> Result<(), Error> {
    // FIXME: provide an option to enable this, because it is super annoying
    // in vim when accidentally pressing shift-space and it emits a sequence
    // that undoes some number of commands
    if false {
        write!(buf, "\x1b[{};{}u", c as u32, 1 + encode_modifiers(mods))?;
    }
    Ok(())
}

/// characters that when masked for CTRL could be an ascii control character
/// or could be a key that a user legitimately wants to process in their
/// terminal application
fn is_ambiguous_ascii_ctrl(c: char) -> bool {
    match c {
        'i' | 'I' | 'm' | 'M' | '[' | '{' | '@' => true,
        _ => false,
    }
}

impl TerminalState {
    pub fn new(
        physical_rows: usize,
        physical_cols: usize,
        pixel_width: usize,
        pixel_height: usize,
        config: Arc<dyn TerminalConfiguration>,
    ) -> TerminalState {
        let screen = ScreenOrAlt::new(physical_rows, physical_cols, &config);

        TerminalState {
            config,
            screen,
            pen: CellAttributes::default(),
            cursor: CursorPosition::default(),
            scroll_region: 0..physical_rows as VisibleRowIndex,
            wrap_next: false,
            insert: false,
            application_cursor_keys: false,
            application_keypad: false,
            bracketed_paste: false,
            sgr_mouse: false,
            button_event_mouse: false,
            cursor_visible: true,
            dec_line_drawing_mode: false,
            current_mouse_button: MouseButton::None,
            tabs: TabStop::new(physical_cols, 8),
            title: "wezterm".to_string(),
            palette: None,
            pixel_height,
            pixel_width,
            clipboard: None,
            current_dir: None,
        }
    }

    pub fn set_clipboard(&mut self, clipboard: &Arc<dyn Clipboard>) {
        self.clipboard.replace(Arc::clone(clipboard));
    }

    pub fn get_title(&self) -> &str {
        &self.title
    }

    /// Returns a copy of the palette.
    /// By default we don't keep a copy in the terminal state,
    /// preferring to take the config values from the users
    /// config file and updating to changes live.
    /// However, if they have used dynamic color scheme escape
    /// sequences we'll fork a copy of the palette at that time
    /// so that we can start tracking those changes.
    pub fn palette(&self) -> ColorPalette {
        self.palette
            .as_ref()
            .cloned()
            .unwrap_or_else(|| self.config.color_palette())
    }

    /// Called in response to dynamic color scheme escape sequences.
    /// Will make a copy of the palette from the config file if this
    /// is the first of these escapes we've seen.
    pub fn palette_mut(&mut self) -> &mut ColorPalette {
        if self.palette.is_none() {
            self.palette.replace(self.config.color_palette());
        }
        self.palette.as_mut().unwrap()
    }

    pub fn screen(&self) -> &Screen {
        &self.screen
    }

    pub fn screen_mut(&mut self) -> &mut Screen {
        &mut self.screen
    }

    fn set_clipboard_contents(&self, text: Option<String>) -> anyhow::Result<()> {
        if let Some(clip) = self.clipboard.as_ref() {
            clip.set_contents(text)?;
        }
        Ok(())
    }

    fn mouse_wheel(
        &mut self,
        event: MouseEvent,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Error> {
        let (report_button, key) = match event.button {
            MouseButton::WheelUp(_) => (64, KeyCode::UpArrow),
            MouseButton::WheelDown(_) => (65, KeyCode::DownArrow),
            _ => bail!("unexpected mouse event {:?}", event),
        };

        if self.sgr_mouse {
            writer.write_all(
                format!("\x1b[<{};{};{}M", report_button, event.x + 1, event.y + 1).as_bytes(),
            )?;
        } else if self.screen.is_alt_screen_active() {
            // Send cursor keys instead (equivalent to xterm's alternateScroll mode)
            self.key_down(key, KeyModifiers::default(), writer)?;
        }
        Ok(())
    }

    fn mouse_button_press(
        &mut self,
        event: MouseEvent,
        host: &mut dyn TerminalHost,
    ) -> Result<(), Error> {
        self.current_mouse_button = event.button;
        if let Some(button) = match event.button {
            MouseButton::Left => Some(0),
            MouseButton::Middle => Some(1),
            MouseButton::Right => Some(2),
            _ => None,
        } {
            if self.sgr_mouse {
                host.writer().write_all(
                    format!("\x1b[<{};{};{}M", button, event.x + 1, event.y + 1).as_bytes(),
                )?;
            }
        }

        Ok(())
    }

    fn mouse_button_release(
        &mut self,
        event: MouseEvent,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Error> {
        if self.current_mouse_button != MouseButton::None {
            self.current_mouse_button = MouseButton::None;
            if self.sgr_mouse {
                write!(writer, "\x1b[<3;{};{}m", event.x + 1, event.y + 1)?;
            }
        }

        Ok(())
    }

    fn mouse_move(
        &mut self,
        event: MouseEvent,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Error> {
        if let Some(button) = match (self.current_mouse_button, self.button_event_mouse) {
            (MouseButton::Left, true) => Some(32),
            (MouseButton::Middle, true) => Some(33),
            (MouseButton::Right, true) => Some(34),
            (..) => None,
        } {
            if self.sgr_mouse {
                write!(writer, "\x1b[<{};{};{}M", button, event.x + 1, event.y + 1)?;
            }
        }
        Ok(())
    }

    pub fn mouse_event(
        &mut self,
        mut event: MouseEvent,
        host: &mut dyn TerminalHost,
    ) -> Result<(), Error> {
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
            } => self.mouse_wheel(event, host.writer()),
            MouseEvent {
                kind: MouseEventKind::Press,
                ..
            } => self.mouse_button_press(event, host),
            MouseEvent {
                kind: MouseEventKind::Release,
                ..
            } => self.mouse_button_release(event, host.writer()),
            MouseEvent {
                kind: MouseEventKind::Move,
                ..
            } => self.mouse_move(event, host.writer()),
        }
    }

    pub fn is_mouse_grabbed(&self) -> bool {
        self.sgr_mouse
    }

    pub fn bracketed_paste_enabled(&self) -> bool {
        self.bracketed_paste
    }

    /// Send text to the terminal that is the result of pasting.
    /// If bracketed paste mode is enabled, the paste is enclosed
    /// in the bracketing, otherwise it is fed to the pty as-is.
    pub fn send_paste(&mut self, text: &str, writer: &mut dyn std::io::Write) -> Result<(), Error> {
        if self.bracketed_paste {
            let buf = format!("\x1b[200~{}\x1b[201~", text);
            writer.write_all(buf.as_bytes())?;
        } else {
            writer.write_all(text.as_bytes())?;
        }
        Ok(())
    }

    /// Processes a key_down event generated by the gui/render layer
    /// that is embedding the Terminal.  This method translates the
    /// keycode into a sequence of bytes to send to the slave end
    /// of the pty via the `Write`-able object provided by the caller.
    #[allow(clippy::cognitive_complexity)]
    pub fn key_down(
        &mut self,
        key: KeyCode,
        mods: KeyModifiers,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Error> {
        use crate::KeyCode::*;

        let key = key.normalize_shift_to_upper_case(mods);
        // Normalize the modifier state for Char's that are uppercase; remove
        // the SHIFT modifier so that reduce ambiguity below
        let mods = match key {
            Char(c)
                if (c.is_ascii_punctuation() || c.is_ascii_uppercase())
                    && mods.contains(KeyModifiers::SHIFT) =>
            {
                mods & !KeyModifiers::SHIFT
            }
            _ => mods,
        };

        // Normalize Backspace and Delete
        let key = match key {
            Char('\x7f') => Delete,
            Char('\x08') => Backspace,
            c => c,
        };

        let mut buf = String::new();

        // TODO: also respect self.application_keypad

        let to_send = match key {
            Char(c) if is_ambiguous_ascii_ctrl(c) && mods.contains(KeyModifiers::CTRL) => {
                csi_u_encode(&mut buf, c, mods)?;
                buf.as_str()
            }
            Char(c) if c.is_ascii_uppercase() && mods.contains(KeyModifiers::CTRL) => {
                csi_u_encode(&mut buf, c, mods)?;
                buf.as_str()
            }

            Char(c)
                if (c.is_ascii_alphanumeric() || c.is_ascii_punctuation())
                    && mods.contains(KeyModifiers::CTRL) =>
            {
                let c = ((c as u8) & 0x1f) as char;
                if mods.contains(KeyModifiers::ALT) {
                    buf.push(0x1b as char);
                }
                buf.push(c);
                buf.as_str()
            }

            // When alt is pressed, send escape first to indicate to the peer that
            // ALT is pressed.  We do this only for ascii alnum characters because
            // eg: on macOS generates altgr style glyphs and keeps the ALT key
            // in the modifier set.  This confuses eg: zsh which then just displays
            // <fffffffff> as the input, so we want to avoid that.
            Char(c)
                if (c.is_ascii_alphanumeric() || c.is_ascii_punctuation())
                    && mods.contains(KeyModifiers::ALT) =>
            {
                buf.push(0x1b as char);
                buf.push(c);
                buf.as_str()
            }

            Enter | Escape | Backspace => {
                let c = match key {
                    Enter => '\r',
                    Escape => '\x1b',
                    // Backspace sends the default VERASE which is confusingly
                    // the DEL ascii codepoint
                    Backspace => '\x7f',
                    _ => unreachable!(),
                };
                if mods.contains(KeyModifiers::SHIFT) || mods.contains(KeyModifiers::CTRL) {
                    csi_u_encode(&mut buf, c, mods)?;
                } else {
                    if mods.contains(KeyModifiers::ALT) && key != Escape {
                        buf.push(0x1b as char);
                    }
                    buf.push(c);
                }
                buf.as_str()
            }

            Tab => {
                if mods.contains(KeyModifiers::ALT) {
                    buf.push(0x1b as char);
                }
                let mods = mods & !KeyModifiers::ALT;
                if mods == KeyModifiers::CTRL {
                    buf.push_str("\x1b[9;5u");
                } else if mods == KeyModifiers::CTRL | KeyModifiers::SHIFT {
                    buf.push_str("\x1b[1;5Z");
                } else if mods == KeyModifiers::SHIFT {
                    buf.push_str("\x1b[Z");
                } else {
                    buf.push('\t');
                }
                buf.as_str()
            }

            Char(c) => {
                if mods.is_empty() {
                    buf.push(c);
                } else {
                    csi_u_encode(&mut buf, c, mods)?;
                }
                buf.as_str()
            }

            Home
            | End
            | UpArrow
            | DownArrow
            | RightArrow
            | LeftArrow
            | ApplicationUpArrow
            | ApplicationDownArrow
            | ApplicationRightArrow
            | ApplicationLeftArrow => {
                let (force_app, c) = match key {
                    UpArrow => (false, 'A'),
                    DownArrow => (false, 'B'),
                    RightArrow => (false, 'C'),
                    LeftArrow => (false, 'D'),
                    Home => (false, 'H'),
                    End => (false, 'F'),
                    ApplicationUpArrow => (true, 'A'),
                    ApplicationDownArrow => (true, 'B'),
                    ApplicationRightArrow => (true, 'C'),
                    ApplicationLeftArrow => (true, 'D'),
                    _ => unreachable!(),
                };

                let csi_or_ss3 = if force_app || self.application_cursor_keys {
                    // Use SS3 in application mode
                    "\x1bO"
                } else {
                    // otherwise use regular CSI
                    "\x1b["
                };

                if mods.contains(KeyModifiers::SHIFT) || mods.contains(KeyModifiers::CTRL) {
                    write!(buf, "{}1;{}{}", csi_or_ss3, 1 + encode_modifiers(mods), c)?;
                } else {
                    if mods.contains(KeyModifiers::ALT) {
                        buf.push(0x1b as char);
                    }
                    write!(buf, "{}{}", csi_or_ss3, c)?;
                }
                buf.as_str()
            }

            PageUp | PageDown | Insert | Delete => {
                let c = match key {
                    Insert => 2,
                    Delete => 3,
                    PageUp => 5,
                    PageDown => 6,
                    _ => unreachable!(),
                };

                if mods.contains(KeyModifiers::SHIFT) || mods.contains(KeyModifiers::CTRL) {
                    write!(buf, "\x1b[{};{}~", c, 1 + encode_modifiers(mods))?;
                } else {
                    if mods.contains(KeyModifiers::ALT) {
                        buf.push(0x1b as char);
                    }
                    write!(buf, "\x1b[{}~", c)?;
                }
                buf.as_str()
            }

            Function(n) => {
                if mods.is_empty() && n < 5 {
                    // F1-F4 are encoded using SS3 if there are no modifiers
                    match n {
                        1 => "\x1bOP",
                        2 => "\x1bOQ",
                        3 => "\x1bOR",
                        4 => "\x1bOS",
                        _ => unreachable!("wat?"),
                    }
                } else {
                    // Higher numbered F-keys plus modified F-keys are encoded
                    // using CSI instead of SS3.
                    let intro = match n {
                        1 => "\x1b[11",
                        2 => "\x1b[12",
                        3 => "\x1b[13",
                        4 => "\x1b[14",
                        5 => "\x1b[15",
                        6 => "\x1b[17",
                        7 => "\x1b[18",
                        8 => "\x1b[19",
                        9 => "\x1b[20",
                        10 => "\x1b[21",
                        11 => "\x1b[23",
                        12 => "\x1b[24",
                        _ => bail!("unhandled fkey number {}", n),
                    };
                    write!(buf, "{};{}~", intro, 1 + encode_modifiers(mods))?;
                    buf.as_str()
                }
            }

            // TODO: emit numpad sequences
            Numpad0 | Numpad1 | Numpad2 | Numpad3 | Numpad4 | Numpad5 | Numpad6 | Numpad7
            | Numpad8 | Numpad9 | Multiply | Add | Separator | Subtract | Decimal | Divide => "",

            // Modifier keys pressed on their own don't expand to anything
            Control | LeftControl | RightControl | Alt | LeftAlt | RightAlt | Menu | LeftMenu
            | RightMenu | Super | Hyper | Shift | LeftShift | RightShift | Meta | LeftWindows
            | RightWindows | NumLock | ScrollLock => "",

            Cancel | Clear | Pause | CapsLock | Select | Print | PrintScreen | Execute | Help
            | Applications | Sleep | BrowserBack | BrowserForward | BrowserRefresh
            | BrowserStop | BrowserSearch | BrowserFavorites | BrowserHome | VolumeMute
            | VolumeDown | VolumeUp | MediaNextTrack | MediaPrevTrack | MediaStop
            | MediaPlayPause | InternalPasteStart | InternalPasteEnd => "",
        };

        // debug!("sending {:?}, {:?}", to_send, key);
        writer.write_all(to_send.as_bytes())?;

        Ok(())
    }

    pub fn resize(
        &mut self,
        physical_rows: usize,
        physical_cols: usize,
        pixel_width: usize,
        pixel_height: usize,
    ) {
        // Compute the change in the height of the viewport;
        // we'll use this to adjust the cursor position below.
        let y_delta = (physical_rows as i64) - (self.screen().physical_rows as i64);
        self.screen.resize(physical_rows, physical_cols);
        self.scroll_region = 0..physical_rows as i64;
        self.pixel_height = pixel_height;
        self.pixel_width = pixel_width;
        self.tabs.resize(physical_cols);
        // Ensure that the cursor is within the new bounds of the screen.
        // If we made the window smaller then we will have scrolled the
        // viewport contents up by the delta.
        // If we've made it larger then we do not want to adjust the
        // cursor position
        self.set_cursor_pos(&Position::Relative(0), &Position::Relative(y_delta.min(0)));
    }

    /// Clear the dirty flag for all dirty lines
    pub fn clean_dirty_lines(&mut self) {
        let screen = self.screen_mut();
        for line in &mut screen.lines {
            line.clear_dirty();
        }
    }

    /// When dealing with selection, mark a range of lines as dirty
    pub fn make_all_lines_dirty(&mut self) {
        let screen = self.screen_mut();
        for line in &mut screen.lines {
            line.set_dirty();
        }
    }

    /// Returns the 0-based cursor position relative to the top left of
    /// the visible screen
    pub fn cursor_pos(&self) -> CursorPosition {
        CursorPosition {
            x: self.cursor.x,
            y: self.cursor.y,
            shape: if self.cursor_visible {
                self.cursor.shape
            } else {
                CursorShape::Hidden
            },
        }
    }

    /// Sets the cursor position. x and y are 0-based and relative to the
    /// top left of the visible screen.
    /// TODO: DEC origin mode impacts the interpreation of these
    fn set_cursor_pos(&mut self, x: &Position, y: &Position) {
        let x = match *x {
            Position::Relative(x) => (self.cursor.x as i64 + x).max(0),
            Position::Absolute(x) => x,
        };
        let y = match *y {
            Position::Relative(y) => (self.cursor.y + y).max(0),
            Position::Absolute(y) => y,
        };

        let rows = self.screen().physical_rows;
        let cols = self.screen().physical_cols;
        let old_y = self.cursor.y;
        let new_y = y.min(rows as i64 - 1);

        self.cursor.x = x.min(cols as i64 - 1) as usize;
        self.cursor.y = new_y;
        self.wrap_next = false;

        let screen = self.screen_mut();
        screen.dirty_line(old_y);
        screen.dirty_line(new_y);
    }

    fn scroll_up(&mut self, num_rows: usize) {
        let scroll_region = self.scroll_region.clone();
        self.screen_mut().scroll_up(&scroll_region, num_rows)
    }

    fn scroll_down(&mut self, num_rows: usize) {
        let scroll_region = self.scroll_region.clone();
        self.screen_mut().scroll_down(&scroll_region, num_rows)
    }

    fn new_line(&mut self, move_to_first_column: bool) {
        let x = if move_to_first_column {
            0
        } else {
            self.cursor.x
        };
        let y = self.cursor.y;
        let y = if y == self.scroll_region.end - 1 {
            self.scroll_up(1);
            y
        } else {
            y + 1
        };
        self.set_cursor_pos(&Position::Absolute(x as i64), &Position::Absolute(y as i64));
    }

    /// Moves the cursor down one line in the same column.
    /// If the cursor is at the bottom margin, the page scrolls up.
    fn c1_index(&mut self) {
        let y = self.cursor.y;
        let y = if y == self.scroll_region.end - 1 {
            self.scroll_up(1);
            y
        } else {
            y + 1
        };
        self.set_cursor_pos(&Position::Relative(0), &Position::Absolute(y as i64));
    }

    /// Moves the cursor to the first position on the next line.
    /// If the cursor is at the bottom margin, the page scrolls up.
    fn c1_nel(&mut self) {
        self.new_line(true);
    }

    /// Sets a horizontal tab stop at the column where the cursor is.
    fn c1_hts(&mut self) {
        self.tabs.set_tab_stop(self.cursor.x);
    }

    /// Moves the cursor to the next tab stop. If there are no more tab stops,
    /// the cursor moves to the right margin. HT does not cause text to auto
    /// wrap.
    fn c0_horizontal_tab(&mut self) {
        let x = match self.tabs.find_next_tab_stop(self.cursor.x) {
            Some(x) => x,
            None => self.screen().physical_cols - 1,
        };
        self.set_cursor_pos(&Position::Absolute(x as i64), &Position::Relative(0));
    }

    /// Move the cursor up 1 line.  If the position is at the top scroll margin,
    /// scroll the region down.
    fn c1_reverse_index(&mut self) {
        let y = self.cursor.y;
        let y = if y == self.scroll_region.start {
            self.scroll_down(1);
            y
        } else {
            y - 1
        };
        self.set_cursor_pos(&Position::Relative(0), &Position::Absolute(y as i64));
    }

    fn set_hyperlink(&mut self, link: Option<Hyperlink>) {
        self.pen.hyperlink = match link {
            Some(hyperlink) => Some(Arc::new(hyperlink)),
            None => None,
        }
    }

    fn set_image(&mut self, image: ITermFileData) {
        if !image.inline {
            error!(
                "Ignoring file download request name={:?} size={}",
                image.name,
                image.data.len()
            );
            return;
        }

        // Decode the image data
        let decoded_image = match image::load_from_memory(&image.data) {
            Ok(im) => im,
            Err(e) => {
                error!(
                    "Unable to decode image: {}: size={} {:?}",
                    e,
                    image.data.len(),
                    image
                );
                return;
            }
        };

        // Figure out the dimensions.
        let physical_cols = self.screen().physical_cols;
        let physical_rows = self.screen().physical_rows;
        let cell_pixel_width = self.pixel_width / physical_cols;
        let cell_pixel_height = self.pixel_height / physical_rows;

        let width = image.width.to_pixels(cell_pixel_width, physical_cols);
        let height = image.height.to_pixels(cell_pixel_height, physical_rows);

        // Compute any Automatic dimensions
        let (width, height) = match (width, height) {
            (None, None) => (
                decoded_image.width() as usize,
                decoded_image.height() as usize,
            ),
            (Some(w), None) => {
                let scale = decoded_image.width() as f32 / w as f32;
                let h = decoded_image.height() as f32 * scale;
                (w, h as usize)
            }
            (None, Some(h)) => {
                let scale = decoded_image.height() as f32 / h as f32;
                let w = decoded_image.width() as f32 * scale;
                (w as usize, h)
            }
            (Some(w), Some(h)) => (w, h),
        };

        let width_in_cells = width / cell_pixel_width;
        let height_in_cells = height / cell_pixel_height;

        // TODO: defer this to the actual renderer
        /*
        let available_pixel_width = width_in_cells * cell_pixel_width;
        let available_pixel_height = height_in_cells * cell_pixel_height;

        let resized_image = if image.preserve_aspect_ratio {
            let resized = decoded_image.resize(
                available_pixel_width as u32,
                available_pixel_height as u32,
                image::FilterType::Lanczos3,
            );
            // Pad with black bars to preserve aspect ratio
            // Assumption: new_rgba8 returns black/transparent pixels by default.
            let dest = DynamicImage::new_rgba8(available_pixel_width, available_pixel_height);
            dest.copy_from(resized, 0, 0);
            dest
        } else {
            decoded_image.resize_exact(
                available_pixel_width as u32,
                available_pixel_height as u32,
                image::FilterType::Lanczos3,
            )
        };
        */

        let image_data = Arc::new(ImageData::with_raw_data(image.data));

        let mut ypos = NotNan::new(0.0).unwrap();
        let cursor_x = self.cursor.x;
        let x_delta = 1.0 / (width as f32 / (self.pixel_width as f32 / physical_cols as f32));
        let y_delta = 1.0 / (height as f32 / (self.pixel_height as f32 / physical_rows as f32));
        debug!(
            "image is {}x{} cells, {}x{} pixels",
            width_in_cells, height_in_cells, width, height
        );
        for _ in 0..height_in_cells {
            let mut xpos = NotNan::new(0.0).unwrap();
            let cursor_y = self.cursor.y;
            debug!(
                "setting cells for y={} x=[{}..{}]",
                cursor_y,
                cursor_x,
                cursor_x + width_in_cells
            );
            for x in 0..width_in_cells {
                self.screen_mut().set_cell(
                    cursor_x + x,
                    cursor_y, // + y as VisibleRowIndex,
                    &Cell::new(
                        ' ',
                        CellAttributes::default()
                            .set_image(Some(Box::new(ImageCell::new(
                                TextureCoordinate::new(xpos, ypos),
                                TextureCoordinate::new(xpos + x_delta, ypos + y_delta),
                                image_data.clone(),
                            ))))
                            .clone(),
                    ),
                );
                xpos += x_delta;
            }
            ypos += y_delta;
            self.new_line(false);
        }

        // FIXME: check cursor positioning in iterm
        /*
        self.set_cursor_pos(
            &Position::Relative(width_in_cells as i64),
            &Position::Relative(-(height_in_cells as i64)),
        );
        */
    }

    fn perform_device(&mut self, dev: Device, host: &mut dyn TerminalHost) {
        match dev {
            Device::DeviceAttributes(a) => error!("unhandled: {:?}", a),
            Device::SoftReset => {
                self.pen = CellAttributes::default();
                // TODO: see https://vt100.net/docs/vt510-rm/DECSTR.html
            }
            Device::RequestPrimaryDeviceAttributes => {
                host.writer().write(DEVICE_IDENT).ok();
            }
            Device::RequestSecondaryDeviceAttributes => {
                host.writer().write(b"\x1b[>0;0;0c").ok();
            }
            Device::StatusReport => {
                host.writer().write(b"\x1b[0n").ok();
            }
        }
    }

    fn perform_csi_mode(&mut self, mode: Mode) {
        match mode {
            Mode::SetDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::StartBlinkingCursor,
            ))
            | Mode::ResetDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::StartBlinkingCursor,
            )) => {}

            Mode::SetMode(TerminalMode::Code(TerminalModeCode::Insert)) => {
                self.insert = true;
            }
            Mode::ResetMode(TerminalMode::Code(TerminalModeCode::Insert)) => {
                self.insert = false;
            }

            Mode::SetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::BracketedPaste)) => {
                self.bracketed_paste = true;
            }
            Mode::ResetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::BracketedPaste)) => {
                self.bracketed_paste = false;
            }

            Mode::SetDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::EnableAlternateScreen,
            )) => {
                if !self.screen.is_alt_screen_active() {
                    self.screen.activate_alt_screen();
                }
            }
            Mode::ResetDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::EnableAlternateScreen,
            )) => {
                if self.screen.is_alt_screen_active() {
                    self.screen.activate_primary_screen();
                }
            }

            Mode::SetDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::ApplicationCursorKeys,
            )) => {
                self.application_cursor_keys = true;
            }
            Mode::ResetDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::ApplicationCursorKeys,
            )) => {
                self.application_cursor_keys = false;
            }

            Mode::SetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::ShowCursor)) => {
                self.cursor_visible = true;
            }
            Mode::ResetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::ShowCursor)) => {
                self.cursor_visible = false;
            }

            Mode::SetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::MouseTracking))
            | Mode::ResetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::MouseTracking)) => {
            }

            Mode::SetDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::HighlightMouseTracking,
            ))
            | Mode::ResetDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::HighlightMouseTracking,
            )) => {}

            Mode::SetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::ButtonEventMouse)) => {
                self.button_event_mouse = true;
            }
            Mode::ResetDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::ButtonEventMouse,
            )) => {
                self.button_event_mouse = false;
            }

            Mode::SetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::AnyEventMouse))
            | Mode::ResetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::AnyEventMouse)) => {
            }

            Mode::SetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::SGRMouse)) => {
                self.sgr_mouse = true;
            }
            Mode::ResetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::SGRMouse)) => {
                self.sgr_mouse = false;
            }

            Mode::SetDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::ClearAndEnableAlternateScreen,
            )) => {
                if !self.screen.is_alt_screen_active() {
                    self.save_cursor();
                    self.screen.activate_alt_screen();
                    self.set_cursor_pos(&Position::Absolute(0), &Position::Absolute(0));
                    self.erase_in_display(EraseInDisplay::EraseDisplay);
                }
            }
            Mode::ResetDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::ClearAndEnableAlternateScreen,
            )) => {
                if self.screen.is_alt_screen_active() {
                    self.screen.activate_primary_screen();
                    self.restore_cursor();
                }
            }
            Mode::SaveDecPrivateMode(DecPrivateMode::Code(_))
            | Mode::RestoreDecPrivateMode(DecPrivateMode::Code(_)) => {
                error!("save/restore dec mode unimplemented")
            }

            Mode::SetDecPrivateMode(DecPrivateMode::Unspecified(n))
            | Mode::ResetDecPrivateMode(DecPrivateMode::Unspecified(n))
            | Mode::SaveDecPrivateMode(DecPrivateMode::Unspecified(n))
            | Mode::RestoreDecPrivateMode(DecPrivateMode::Unspecified(n)) => {
                error!("unhandled DecPrivateMode {}", n);
            }

            Mode::SetMode(TerminalMode::Unspecified(n))
            | Mode::ResetMode(TerminalMode::Unspecified(n)) => {
                error!("unhandled TerminalMode {}", n);
            }

            Mode::SetMode(m) | Mode::ResetMode(m) => {
                error!("unhandled TerminalMode {:?}", m);
            }
        }
    }

    fn checksum_rectangle(&mut self, left: u32, top: u32, right: u32, bottom: u32) -> u16 {
        let screen = self.screen_mut();
        let mut checksum = 0;
        debug!(
            "checksum left={} top={} right={} bottom={}",
            left, top, right, bottom
        );
        for y in top..=bottom {
            let line_idx = screen.phys_row(VisibleRowIndex::from(y));
            let line = screen.line_mut(line_idx);
            for (col, cell) in line.cells().iter().enumerate().skip(left as usize) {
                if col > right as usize {
                    break;
                }

                let ch = cell.str().chars().nth(0).unwrap() as u32;
                debug!("y={} col={} ch={:x} cell={:?}", y, col, ch, cell);

                checksum += u16::from(ch as u8);
            }
        }
        checksum
    }

    fn perform_csi_window(&mut self, window: Window, host: &mut dyn TerminalHost) {
        match window {
            Window::ReportTextAreaSizeCells => {
                let screen = self.screen();
                let height = Some(screen.physical_rows as i64);
                let width = Some(screen.physical_cols as i64);

                let response = Window::ResizeWindowCells { width, height };
                write!(host.writer(), "{}", CSI::Window(response)).ok();
            }
            Window::ChecksumRectangularArea {
                request_id,
                top,
                left,
                bottom,
                right,
                ..
            } => {
                let checksum = self.checksum_rectangle(
                    left.as_zero_based(),
                    top.as_zero_based(),
                    right.as_zero_based(),
                    bottom.as_zero_based(),
                );
                write!(host.writer(), "\x1bP{}!~{:04x}\x1b\\", request_id, checksum).ok();
            }
            Window::Iconify | Window::DeIconify => {}
            Window::PopIconAndWindowTitle
            | Window::PopWindowTitle
            | Window::PopIconTitle
            | Window::PushIconAndWindowTitle
            | Window::PushIconTitle
            | Window::PushWindowTitle => {}
            _ => error!("unhandled Window CSI {:?}", window),
        }
    }

    fn erase_in_display(&mut self, erase: EraseInDisplay) {
        let cy = self.cursor.y;
        let pen = self.pen.clone_sgr_only();
        let rows = self.screen().physical_rows as VisibleRowIndex;
        let col_range = 0..usize::max_value();
        let row_range = match erase {
            EraseInDisplay::EraseToEndOfDisplay => {
                self.perform_csi_edit(Edit::EraseInLine(EraseInLine::EraseToEndOfLine));
                cy + 1..rows
            }
            EraseInDisplay::EraseToStartOfDisplay => {
                self.perform_csi_edit(Edit::EraseInLine(EraseInLine::EraseToStartOfLine));
                0..cy
            }
            EraseInDisplay::EraseDisplay => 0..rows,
            EraseInDisplay::EraseScrollback => {
                error!("TODO: ed: no support for xterm Erase Saved Lines yet");
                return;
            }
        };

        {
            let screen = self.screen_mut();
            for y in row_range.clone() {
                screen.clear_line(y, col_range.clone(), &pen);
            }
        }
    }

    fn perform_csi_edit(&mut self, edit: Edit) {
        match edit {
            Edit::DeleteCharacter(n) => {
                let y = self.cursor.y;
                let x = self.cursor.x;
                let limit = (x + n as usize).min(self.screen().physical_cols);
                {
                    let screen = self.screen_mut();
                    for _ in x..limit as usize {
                        screen.erase_cell(x, y);
                    }
                }
            }
            Edit::DeleteLine(n) => {
                if self.scroll_region.contains(&self.cursor.y) {
                    let scroll_region = self.cursor.y..self.scroll_region.end;
                    self.screen_mut().scroll_up(&scroll_region, n as usize);
                }
            }
            Edit::EraseCharacter(n) => {
                let y = self.cursor.y;
                let x = self.cursor.x;
                let limit = (x + n as usize).min(self.screen().physical_cols);
                {
                    let blank = Cell::new(' ', self.pen.clone_sgr_only());
                    let screen = self.screen_mut();
                    for x in x..limit as usize {
                        screen.set_cell(x, y, &blank);
                    }
                }
            }

            Edit::EraseInLine(erase) => {
                let cx = self.cursor.x;
                let cy = self.cursor.y;
                let pen = self.pen.clone_sgr_only();
                let cols = self.screen().physical_cols;
                let range = match erase {
                    EraseInLine::EraseToEndOfLine => cx..cols,
                    EraseInLine::EraseToStartOfLine => 0..cx,
                    EraseInLine::EraseLine => 0..cols,
                };

                self.screen_mut().clear_line(cy, range.clone(), &pen);
            }
            Edit::InsertCharacter(n) => {
                let y = self.cursor.y;
                let x = self.cursor.x;
                // TODO: this limiting behavior may not be correct.  There's also a
                // SEM sequence that impacts the scope of ICH and ECH to consider.
                let limit = (x + n as usize).min(self.screen().physical_cols);
                {
                    let screen = self.screen_mut();
                    for x in x..limit as usize {
                        screen.insert_cell(x, y);
                    }
                }
            }
            Edit::InsertLine(n) => {
                if self.scroll_region.contains(&self.cursor.y) {
                    let scroll_region = self.cursor.y..self.scroll_region.end;
                    self.screen_mut().scroll_down(&scroll_region, n as usize);
                }
            }
            Edit::ScrollDown(n) => self.scroll_down(n as usize),
            Edit::ScrollUp(n) => self.scroll_up(n as usize),
            Edit::EraseInDisplay(erase) => self.erase_in_display(erase),
            Edit::Repeat(n) => {
                let y = self.cursor.y;
                let x = self.cursor.x;
                let to_copy = x.saturating_sub(1);
                let screen = self.screen_mut();
                let line_idx = screen.phys_row(y);
                let line = screen.line_mut(line_idx);
                if let Some(cell) = line.cells().get(to_copy).cloned() {
                    line.fill_range(x..=x + n as usize, &cell);
                    self.set_cursor_pos(&Position::Relative(i64::from(n)), &Position::Relative(0))
                }
            }
        }
    }

    fn perform_csi_cursor(&mut self, cursor: Cursor, host: &mut dyn TerminalHost) {
        match cursor {
            Cursor::SetTopAndBottomMargins { top, bottom } => {
                let rows = self.screen().physical_rows;
                let mut top = i64::from(top.as_zero_based()).min(rows as i64 - 1).max(0);
                let mut bottom = i64::from(bottom.as_zero_based())
                    .min(rows as i64 - 1)
                    .max(0);
                if top > bottom {
                    std::mem::swap(&mut top, &mut bottom);
                }
                self.scroll_region = top..bottom + 1;
            }
            Cursor::ForwardTabulation(n) => {
                for _ in 0..n {
                    self.c0_horizontal_tab();
                }
            }
            Cursor::BackwardTabulation(_) => {}
            Cursor::TabulationClear(_) => {}
            Cursor::TabulationControl(_) => {}
            Cursor::LineTabulation(_) => {}

            Cursor::Left(n) => {
                self.set_cursor_pos(&Position::Relative(-(i64::from(n))), &Position::Relative(0))
            }
            Cursor::Right(n) => {
                self.set_cursor_pos(&Position::Relative(i64::from(n)), &Position::Relative(0))
            }
            Cursor::Up(n) => {
                self.set_cursor_pos(&Position::Relative(0), &Position::Relative(-(i64::from(n))))
            }
            Cursor::Down(n) => {
                self.set_cursor_pos(&Position::Relative(0), &Position::Relative(i64::from(n)))
            }
            Cursor::CharacterAndLinePosition { line, col } | Cursor::Position { line, col } => self
                .set_cursor_pos(
                    &Position::Absolute(i64::from(col.as_zero_based())),
                    &Position::Absolute(i64::from(line.as_zero_based())),
                ),
            Cursor::CharacterAbsolute(col) | Cursor::CharacterPositionAbsolute(col) => self
                .set_cursor_pos(
                    &Position::Absolute(i64::from(col.as_zero_based())),
                    &Position::Relative(0),
                ),
            Cursor::CharacterPositionBackward(col) => self.set_cursor_pos(
                &Position::Relative(-(i64::from(col))),
                &Position::Relative(0),
            ),
            Cursor::CharacterPositionForward(col) => {
                self.set_cursor_pos(&Position::Relative(i64::from(col)), &Position::Relative(0))
            }
            Cursor::LinePositionAbsolute(line) => self.set_cursor_pos(
                &Position::Relative(0),
                &Position::Absolute((i64::from(line)).saturating_sub(1)),
            ),
            Cursor::LinePositionBackward(line) => self.set_cursor_pos(
                &Position::Relative(0),
                &Position::Relative(-(i64::from(line))),
            ),
            Cursor::LinePositionForward(line) => {
                self.set_cursor_pos(&Position::Relative(0), &Position::Relative(i64::from(line)))
            }
            Cursor::NextLine(n) => {
                for _ in 0..n {
                    self.new_line(true);
                }
            }
            Cursor::PrecedingLine(n) => {
                self.set_cursor_pos(&Position::Absolute(0), &Position::Relative(-(i64::from(n))))
            }
            Cursor::ActivePositionReport { .. } => {
                // This is really a response from the terminal, and
                // we don't need to process it as a terminal command
            }
            Cursor::RequestActivePositionReport => {
                let line = OneBased::from_zero_based(self.cursor.y as u32);
                let col = OneBased::from_zero_based(self.cursor.x as u32);
                let report = CSI::Cursor(Cursor::ActivePositionReport { line, col });
                write!(host.writer(), "{}", report).ok();
            }
            Cursor::SaveCursor => self.save_cursor(),
            Cursor::RestoreCursor => self.restore_cursor(),
            Cursor::CursorStyle(style) => {
                self.cursor.shape = match style {
                    CursorStyle::Default => CursorShape::Default,
                    CursorStyle::BlinkingBlock => CursorShape::BlinkingBlock,
                    CursorStyle::SteadyBlock => CursorShape::SteadyBlock,
                    CursorStyle::BlinkingUnderline => CursorShape::BlinkingUnderline,
                    CursorStyle::SteadyUnderline => CursorShape::SteadyUnderline,
                    CursorStyle::BlinkingBar => CursorShape::BlinkingBar,
                    CursorStyle::SteadyBar => CursorShape::SteadyBar,
                };
                log::debug!("Cursor shape is now {:?}", self.cursor.shape);
            }
        }
    }

    fn save_cursor(&mut self) {
        let saved = SavedCursor {
            position: self.cursor,
            insert: self.insert,
            wrap_next: self.wrap_next,
        };
        debug!(
            "saving cursor {:?} is_alt={}",
            saved,
            self.screen.is_alt_screen_active()
        );
        *self.screen.saved_cursor() = Some(saved);
    }
    fn restore_cursor(&mut self) {
        let saved = self.screen.saved_cursor().unwrap_or_else(|| SavedCursor {
            position: CursorPosition::default(),
            insert: false,
            wrap_next: false,
        });
        debug!(
            "restore cursor {:?} is_alt={}",
            saved,
            self.screen.is_alt_screen_active()
        );
        let x = saved.position.x;
        let y = saved.position.y;
        self.set_cursor_pos(&Position::Absolute(x as i64), &Position::Absolute(y));
        self.cursor.shape = saved.position.shape;
        self.wrap_next = saved.wrap_next;
        self.insert = saved.insert;
    }

    fn perform_csi_sgr(&mut self, sgr: Sgr) {
        debug!("{:?}", sgr);
        match sgr {
            Sgr::Reset => {
                let link = self.pen.hyperlink.take();
                self.pen = CellAttributes::default();
                self.pen.hyperlink = link;
            }
            Sgr::Intensity(intensity) => {
                self.pen.set_intensity(intensity);
            }
            Sgr::Underline(underline) => {
                self.pen.set_underline(underline);
            }
            Sgr::Blink(blink) => {
                self.pen.set_blink(blink);
            }
            Sgr::Italic(italic) => {
                self.pen.set_italic(italic);
            }
            Sgr::Inverse(inverse) => {
                self.pen.set_reverse(inverse);
            }
            Sgr::Invisible(invis) => {
                self.pen.set_invisible(invis);
            }
            Sgr::StrikeThrough(strike) => {
                self.pen.set_strikethrough(strike);
            }
            Sgr::Foreground(col) => {
                self.pen.set_foreground(col);
            }
            Sgr::Background(col) => {
                self.pen.set_background(col);
            }
            Sgr::Font(_) => {}
        }
    }
}

/// A helper struct for implementing `vtparse::VTActor` while compartmentalizing
/// the terminal state and the embedding/host terminal interface
pub(crate) struct Performer<'a> {
    pub state: &'a mut TerminalState,
    pub host: &'a mut dyn TerminalHost,
    print: Option<String>,
}

impl<'a> Deref for Performer<'a> {
    type Target = TerminalState;

    fn deref(&self) -> &TerminalState {
        self.state
    }
}

impl<'a> DerefMut for Performer<'a> {
    fn deref_mut(&mut self) -> &mut TerminalState {
        &mut self.state
    }
}

impl<'a> Drop for Performer<'a> {
    fn drop(&mut self) {
        self.flush_print();
    }
}

impl<'a> Performer<'a> {
    pub fn new(state: &'a mut TerminalState, host: &'a mut dyn TerminalHost) -> Self {
        Self {
            state,
            host,
            print: None,
        }
    }

    fn flush_print(&mut self) {
        let p = match self.print.take() {
            Some(s) => s,
            None => return,
        };

        let mut x_offset = 0;

        for g in unicode_segmentation::UnicodeSegmentation::graphemes(p.as_str(), true) {
            let g = if self.dec_line_drawing_mode {
                match g {
                    "j" => "┘",
                    "k" => "┐",
                    "l" => "┌",
                    "m" => "└",
                    "n" => "┼",
                    "q" => "─",
                    "t" => "├",
                    "u" => "┤",
                    "v" => "┴",
                    "w" => "┬",
                    "x" => "│",
                    _ => g,
                }
            } else {
                g
            };

            if !self.insert && self.wrap_next {
                self.new_line(true);
            }

            let x = self.cursor.x;
            let y = self.cursor.y;
            let width = self.screen().physical_cols;

            let mut pen = self.pen.clone();
            // the max(1) here is to ensure that we advance to the next cell
            // position for zero-width graphemes.  We want to make sure that
            // they occupy a cell so that we can re-emit them when we output them.
            // If we didn't do this, then we'd effectively filter them out from
            // the model, which seems like a lossy design choice.
            let print_width = unicode_column_width(g).max(1);

            if !self.insert && x + print_width >= width {
                pen.set_wrapped(true);
            }

            let cell = Cell::new_grapheme(g, pen);

            if self.insert {
                let screen = self.screen_mut();
                for _ in x..x + print_width as usize {
                    screen.insert_cell(x + x_offset, y);
                }
            }

            // Assign the cell
            self.screen_mut().set_cell(x + x_offset, y, &cell);

            if self.insert {
                x_offset += print_width;
            } else if x + print_width < width {
                self.cursor.x += print_width;
                self.wrap_next = false;
            } else {
                self.wrap_next = true;
            }
        }
    }

    pub fn perform(&mut self, action: Action) {
        debug!("perform {:?}", action);
        match action {
            Action::Print(c) => self.print(c),
            Action::Control(code) => self.control(code),
            Action::DeviceControl(ctrl) => error!("Unhandled {:?}", ctrl),
            Action::OperatingSystemCommand(osc) => self.osc_dispatch(*osc),
            Action::Esc(esc) => self.esc_dispatch(esc),
            Action::CSI(csi) => self.csi_dispatch(csi),
        }
    }

    /// Draw a character to the screen
    fn print(&mut self, c: char) {
        // We buffer up the chars to increase the chances of correctly grouping graphemes into cells
        self.print.get_or_insert_with(String::new).push(c);
    }

    fn control(&mut self, control: ControlCode) {
        self.flush_print();
        match control {
            ControlCode::LineFeed | ControlCode::VerticalTab | ControlCode::FormFeed => {
                self.new_line(false)
            }
            ControlCode::CarriageReturn => {
                self.set_cursor_pos(&Position::Absolute(0), &Position::Relative(0));
            }
            ControlCode::Backspace => {
                self.set_cursor_pos(&Position::Relative(-1), &Position::Relative(0));
            }
            ControlCode::HorizontalTab => self.c0_horizontal_tab(),
            ControlCode::Bell => error!("Ding! (this is the bell)"),
            _ => error!("unhandled ControlCode {:?}", control),
        }
    }

    fn csi_dispatch(&mut self, csi: CSI) {
        self.flush_print();
        match csi {
            CSI::Sgr(sgr) => self.state.perform_csi_sgr(sgr),
            CSI::Cursor(cursor) => self.state.perform_csi_cursor(cursor, self.host),
            CSI::Edit(edit) => self.state.perform_csi_edit(edit),
            CSI::Mode(mode) => self.state.perform_csi_mode(mode),
            CSI::Device(dev) => self.state.perform_device(*dev, self.host),
            CSI::Mouse(mouse) => error!("mouse report sent by app? {:?}", mouse),
            CSI::Window(window) => self.state.perform_csi_window(window, self.host),
            CSI::Unspecified(unspec) => {
                error!("unknown unspecified CSI: {:?}", format!("{}", unspec))
            }
        };
    }

    fn esc_dispatch(&mut self, esc: Esc) {
        self.flush_print();
        match esc {
            Esc::Code(EscCode::StringTerminator) => {
                // String Terminator (ST); explicitly has nothing to do here, as its purpose is
                // handled implicitly through a state transition in the vtparse state tables.
            }
            Esc::Code(EscCode::DecApplicationKeyPad) => {
                debug!("DECKPAM on");
                self.application_keypad = true;
            }
            Esc::Code(EscCode::DecNormalKeyPad) => {
                debug!("DECKPAM off");
                self.application_keypad = false;
            }
            Esc::Code(EscCode::ReverseIndex) => self.c1_reverse_index(),
            Esc::Code(EscCode::Index) => self.c1_index(),
            Esc::Code(EscCode::NextLine) => self.c1_nel(),
            Esc::Code(EscCode::HorizontalTabSet) => self.c1_hts(),
            Esc::Code(EscCode::DecLineDrawing) => {
                self.dec_line_drawing_mode = true;
            }
            Esc::Code(EscCode::AsciiCharacterSet) => {
                self.dec_line_drawing_mode = false;
            }
            Esc::Code(EscCode::DecSaveCursorPosition) => self.save_cursor(),
            Esc::Code(EscCode::DecRestoreCursorPosition) => self.restore_cursor(),
            _ => error!("ESC: unhandled {:?}", esc),
        }
    }

    fn osc_dispatch(&mut self, osc: OperatingSystemCommand) {
        self.flush_print();
        match osc {
            OperatingSystemCommand::SetIconNameAndWindowTitle(title)
            | OperatingSystemCommand::SetWindowTitle(title) => {
                self.title = title.clone();
                self.host.set_title(&title);
            }
            OperatingSystemCommand::SetIconName(_) => {}
            OperatingSystemCommand::SetHyperlink(link) => {
                self.set_hyperlink(link);
            }
            OperatingSystemCommand::Unspecified(unspec) => {
                let mut output = String::new();
                write!(&mut output, "Unhandled OSC ").ok();
                for item in unspec {
                    write!(&mut output, " {}", String::from_utf8_lossy(&item)).ok();
                }
                error!("{}", output);
            }

            OperatingSystemCommand::ClearSelection(_) => {
                self.set_clipboard_contents(None).ok();
            }
            OperatingSystemCommand::QuerySelection(_) => {}
            OperatingSystemCommand::SetSelection(_, selection_data) => {
                match self.set_clipboard_contents(Some(selection_data)) {
                    Ok(_) => (),
                    Err(err) => error!("failed to set clipboard in response to OSC 52: {:?}", err),
                }
            }
            OperatingSystemCommand::ITermProprietary(iterm) => match iterm {
                ITermProprietary::File(image) => self.set_image(*image),
                _ => error!("unhandled iterm2: {:?}", iterm),
            },
            OperatingSystemCommand::SystemNotification(message) => {
                error!("Application sends SystemNotification: {}", message);
            }
            OperatingSystemCommand::CurrentWorkingDirectory(url) => {
                let dir = match Url::parse(&url) {
                    Ok(url) if url.scheme() == "file" => Some(url.path().to_string()),
                    Ok(_) | Err(_) => None,
                };
                self.current_dir = dir;
            }
            OperatingSystemCommand::ChangeColorNumber(specs) => {
                error!("ChangeColorNumber: {:?}", specs);
                for pair in specs {
                    match pair.color {
                        ColorOrQuery::Query => {
                            let response =
                                OperatingSystemCommand::ChangeColorNumber(vec![ChangeColorPair {
                                    palette_index: pair.palette_index,
                                    color: ColorOrQuery::Color(
                                        self.palette().colors.0[pair.palette_index as usize],
                                    ),
                                }]);
                            write!(self.host.writer(), "{}", response).ok();
                        }
                        ColorOrQuery::Color(c) => {
                            self.palette_mut().colors.0[pair.palette_index as usize] = c;
                        }
                    }
                }
                self.make_all_lines_dirty();
            }
            OperatingSystemCommand::ChangeDynamicColors(first_color, colors) => {
                error!("ChangeDynamicColors: {:?} {:?}", first_color, colors);
                use termwiz::escape::osc::DynamicColorNumber;
                let mut idx: u8 = first_color as u8;
                for color in colors {
                    let which_color: Option<DynamicColorNumber> = num::FromPrimitive::from_u8(idx);
                    if let Some(which_color) = which_color {
                        macro_rules! set_or_query {
                            ($name:ident) => {
                                match color {
                                    ColorOrQuery::Query => {
                                        let response = OperatingSystemCommand::ChangeDynamicColors(
                                            which_color,
                                            vec![ColorOrQuery::Color(self.palette().$name)],
                                        );
                                        write!(self.host.writer(), "{}", response).ok();
                                    }
                                    ColorOrQuery::Color(c) => self.palette_mut().$name = c,
                                }
                            };
                        }
                        match which_color {
                            DynamicColorNumber::TextForegroundColor => set_or_query!(foreground),
                            DynamicColorNumber::TextBackgroundColor => set_or_query!(background),
                            DynamicColorNumber::TextCursorColor => set_or_query!(cursor_bg),
                            DynamicColorNumber::HighlightForegroundColor => {
                                set_or_query!(selection_fg)
                            }
                            DynamicColorNumber::HighlightBackgroundColor => {
                                set_or_query!(selection_bg)
                            }
                            DynamicColorNumber::MouseForegroundColor
                            | DynamicColorNumber::MouseBackgroundColor
                            | DynamicColorNumber::TektronixForegroundColor
                            | DynamicColorNumber::TektronixBackgroundColor
                            | DynamicColorNumber::TektronixCursorColor => {}
                        }
                    }
                    idx += 1;
                }
                self.make_all_lines_dirty();
            }
        }
    }
}
