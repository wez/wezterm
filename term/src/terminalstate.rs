// The range_plus_one lint can't see when the LHS is not compatible with
// and inclusive range
#![cfg_attr(feature = "cargo-clippy", allow(clippy::range_plus_one))]
use super::*;
use crate::color::{ColorPalette, RgbColor};
use anyhow::bail;
use image::imageops::FilterType;
use image::ImageFormat;
use log::{debug, error};
use num_traits::{FromPrimitive, ToPrimitive};
use ordered_float::NotNan;
use std::collections::HashMap;
use std::fmt::Write;
use std::sync::mpsc::{channel, Sender};
use std::sync::Arc;
use termwiz::escape::csi::{
    Cursor, CursorStyle, DecPrivateMode, DecPrivateModeCode, Device, Edit, EraseInDisplay,
    EraseInLine, Mode, Sgr, TabulationClear, TerminalMode, TerminalModeCode, Window, XtSmGraphics,
    XtSmGraphicsAction, XtSmGraphicsItem, XtSmGraphicsStatus,
};
use termwiz::escape::osc::{
    ChangeColorPair, ColorOrQuery, FinalTermSemanticPrompt, ITermFileData, ITermProprietary,
    Selection,
};
use termwiz::escape::{
    Action, ControlCode, DeviceControlMode, Esc, EscCode, OneBased, OperatingSystemCommand, Sixel,
    SixelData, CSI,
};
use termwiz::image::{ImageCell, ImageData, TextureCoordinate};
use termwiz::surface::{CursorShape, CursorVisibility};
use url::Url;

struct TabStop {
    tabs: Vec<bool>,
    tab_width: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CharSet {
    Ascii,
    Uk,
    DecLineDrawing,
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

    fn find_prev_tab_stop(&self, col: usize) -> Option<usize> {
        for i in (0..col.min(self.tabs.len())).rev() {
            if self.tabs[i] {
                return Some(i);
            }
        }
        None
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

    fn clear(&mut self, to_clear: TabulationClear, col: usize) {
        match to_clear {
            TabulationClear::ClearCharacterTabStopAtActivePosition => {
                if let Some(t) = self.tabs.get_mut(col) {
                    *t = false;
                }
            }
            TabulationClear::ClearAllCharacterTabStops
            | TabulationClear::ClearCharacterTabStopsAtActiveLine => {
                for t in &mut self.tabs {
                    *t = false;
                }
            }
            _ => log::warn!("unhandled TabulationClear {:?}", to_clear),
        }
    }
}

#[derive(Debug, Clone)]
struct SavedCursor {
    position: CursorPosition,
    wrap_next: bool,
    pen: CellAttributes,
    dec_origin_mode: bool,
    g0_charset: CharSet,
    g1_charset: CharSet,
    // TODO: selective_erase when supported
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

    pub fn resize(
        &mut self,
        physical_rows: usize,
        physical_cols: usize,
        cursor: CursorPosition,
    ) -> CursorPosition {
        let cursor_main = self.screen.resize(physical_rows, physical_cols, cursor);
        let cursor_alt = self.alt_screen.resize(physical_rows, physical_cols, cursor);
        if self.alt_screen_is_active {
            cursor_alt
        } else {
            cursor_main
        }
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

/// Manages the state for the terminal
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

    /// https://vt100.net/docs/vt510-rm/DECAWM.html
    dec_auto_wrap: bool,

    /// Reverse Wraparound Mode
    reverse_wraparound_mode: bool,

    /// https://vt100.net/docs/vt510-rm/DECOM.html
    /// When OriginMode is enabled, cursor is constrained to the
    /// scroll region and its position is relative to the scroll
    /// region.
    dec_origin_mode: bool,

    /// The scroll region
    top_and_bottom_margins: Range<VisibleRowIndex>,
    left_and_right_margins: Range<usize>,
    left_and_right_margin_mode: bool,

    /// When set, modifies the sequence of bytes sent for keys
    /// designated as cursor keys.  This includes various navigation
    /// keys.  The code in key_down() is responsible for interpreting this.
    application_cursor_keys: bool,

    dec_ansi_mode: bool,

    /// https://vt100.net/docs/vt3xx-gp/chapter14.html has a discussion
    /// on what sixel scrolling mode does
    sixel_scrolling: bool,
    use_private_color_registers_for_each_graphic: bool,

    /// Graphics mode color register map.
    color_map: HashMap<u16, RgbColor>,

    /// When set, modifies the sequence of bytes sent for keys
    /// in the numeric keypad portion of the keyboard.
    application_keypad: bool,

    /// When set, pasting the clipboard should bracket the data with
    /// designated marker characters.
    bracketed_paste: bool,

    /// Movement events enabled
    any_event_mouse: bool,
    focus_tracking: bool,
    /// SGR style mouse tracking and reporting is enabled
    sgr_mouse: bool,
    mouse_tracking: bool,
    /// Button events enabled
    button_event_mouse: bool,
    current_mouse_button: MouseButton,
    last_mouse_move: Option<MouseEvent>,
    cursor_visible: bool,

    /// Support for US, UK, and DEC Special Graphics
    g0_charset: CharSet,
    g1_charset: CharSet,
    shift_out: bool,

    tabs: TabStop,

    /// The terminal title string (OSC 2)
    title: String,
    /// The icon title string (OSC 1)
    icon_title: Option<String>,

    palette: Option<ColorPalette>,

    pixel_width: usize,
    pixel_height: usize,

    clipboard: Option<Arc<dyn Clipboard>>,
    device_control_handler: Option<Box<dyn DeviceControlHandler>>,
    alert_handler: Option<Box<dyn AlertHandler>>,

    current_dir: Option<Url>,

    term_program: String,
    term_version: String,

    writer: Box<dyn std::io::Write>,

    image_cache: lru::LruCache<[u8; 32], Arc<ImageData>>,
    sixel_scrolls_right: bool,

    user_vars: HashMap<String, String>,
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

/// characters that when masked for CTRL could be an ascii control character
/// or could be a key that a user legitimately wants to process in their
/// terminal application
fn is_ambiguous_ascii_ctrl(c: char) -> bool {
    match c {
        'i' | 'I' | 'm' | 'M' | '[' | '{' | '@' => true,
        _ => false,
    }
}

/// Map c to its Ctrl equivalent.
/// In theory, this mapping is simply translating alpha characters
/// to upper case and then masking them by 0x1f, but xterm inherits
/// some built-in translation from legacy X11 so that are some
/// aliased mappings and a couple that might be technically tied
/// to US keyboard layout (particularly the punctuation characters
/// produced in combination with SHIFT) that may not be 100%
/// the right thing to do here for users with non-US layouts.
fn ctrl_mapping(c: char) -> Option<char> {
    Some(match c {
        '@' | '`' | ' ' | '2' => '\x00',
        'A' | 'a' => '\x01',
        'B' | 'b' => '\x02',
        'C' | 'c' => '\x03',
        'D' | 'd' => '\x04',
        'E' | 'e' => '\x05',
        'F' | 'f' => '\x06',
        'G' | 'g' => '\x07',
        'H' | 'h' => '\x08',
        'I' | 'i' => '\x09',
        'J' | 'j' => '\x0a',
        'K' | 'k' => '\x0b',
        'L' | 'l' => '\x0c',
        'M' | 'm' => '\x0d',
        'N' | 'n' => '\x0e',
        'O' | 'o' => '\x0f',
        'P' | 'p' => '\x10',
        'Q' | 'q' => '\x11',
        'R' | 'r' => '\x12',
        'S' | 's' => '\x13',
        'T' | 't' => '\x14',
        'U' | 'u' => '\x15',
        'V' | 'v' => '\x16',
        'W' | 'w' => '\x17',
        'X' | 'x' => '\x18',
        'Y' | 'y' => '\x19',
        'Z' | 'z' => '\x1a',
        '[' | '3' | '{' => '\x1b',
        '\\' | '4' | '|' => '\x1c',
        ']' | '5' | '}' => '\x1d',
        '^' | '6' | '~' => '\x1e',
        '_' | '7' | '/' => '\x1f',
        '8' | '?' => '\x7f', // `Delete`
        _ => return None,
    })
}

fn default_color_map() -> HashMap<u16, RgbColor> {
    let mut color_map = HashMap::new();
    color_map.insert(0, RgbColor::new_8bpc(0, 0, 0));
    color_map.insert(3, RgbColor::new_8bpc(0, 255, 0));
    color_map
}

/// This struct implements a writer that sends the data across
/// to another thread so that the write side of the terminal
/// processing never blocks.
///
/// This is important for example when processing large pastes into
/// vim.  In that scenario, we can fill up the data pending
/// on vim's input buffer, while it is busy trying to send
/// output to the terminal.  A deadlock is reached because
/// send_paste blocks on the writer, but it is unable to make
/// progress until we're able to read the output from vim.
///
/// We either need input or output to be non-blocking.
/// Output seems safest because we want to be able to exert
/// back-pressure when there is a lot of data to read,
/// and we're in control of the write side, which represents
/// input from the interactive user, or pastes.
struct ThreadedWriter {
    sender: Sender<Vec<u8>>,
}

impl ThreadedWriter {
    fn new(mut writer: Box<dyn std::io::Write + Send>) -> Self {
        let (sender, receiver) = channel::<Vec<u8>>();

        std::thread::spawn(move || {
            while let Ok(buf) = receiver.recv() {
                if writer.write(&buf).is_err() {
                    break;
                }
            }
        });

        Self { sender }
    }
}

impl std::io::Write for ThreadedWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.sender
            .send(buf.to_vec())
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::BrokenPipe, err))?;
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl TerminalState {
    /// Constructs the terminal state.
    /// You generally want the `Terminal` struct rather than this one;
    /// Terminal contains and dereferences to `TerminalState`.
    pub fn new(
        size: TerminalSize,
        config: Arc<dyn TerminalConfiguration>,
        term_program: &str,
        term_version: &str,
        writer: Box<dyn std::io::Write + Send>,
    ) -> TerminalState {
        let writer = Box::new(ThreadedWriter::new(writer));
        let screen = ScreenOrAlt::new(size.physical_rows, size.physical_cols, &config);

        let color_map = default_color_map();

        TerminalState {
            config,
            screen,
            pen: CellAttributes::default(),
            cursor: CursorPosition::default(),
            top_and_bottom_margins: 0..size.physical_rows as VisibleRowIndex,
            left_and_right_margins: 0..size.physical_cols,
            left_and_right_margin_mode: false,
            wrap_next: false,
            // We default auto wrap to true even though the default for
            // a dec terminal is false, because it is more useful this way.
            dec_auto_wrap: true,
            reverse_wraparound_mode: false,
            dec_origin_mode: false,
            insert: false,
            application_cursor_keys: false,
            dec_ansi_mode: false,
            sixel_scrolling: true,
            use_private_color_registers_for_each_graphic: false,
            color_map,
            application_keypad: false,
            bracketed_paste: false,
            focus_tracking: false,
            sgr_mouse: false,
            sixel_scrolls_right: false,
            any_event_mouse: false,
            button_event_mouse: false,
            mouse_tracking: false,
            last_mouse_move: None,
            cursor_visible: true,
            g0_charset: CharSet::Ascii,
            g1_charset: CharSet::DecLineDrawing,
            shift_out: false,
            current_mouse_button: MouseButton::None,
            tabs: TabStop::new(size.physical_cols, 8),
            title: "wezterm".to_string(),
            icon_title: None,
            palette: None,
            pixel_height: size.pixel_height,
            pixel_width: size.pixel_width,
            clipboard: None,
            device_control_handler: None,
            alert_handler: None,
            current_dir: None,
            term_program: term_program.to_string(),
            term_version: term_version.to_string(),
            writer: Box::new(std::io::BufWriter::new(writer)),
            image_cache: lru::LruCache::new(16),
            user_vars: HashMap::new(),
        }
    }

    pub fn set_config(&mut self, config: Arc<dyn TerminalConfiguration>) {
        self.config = config;
    }

    pub fn get_config(&self) -> Arc<dyn TerminalConfiguration> {
        Arc::clone(&self.config)
    }

    pub fn set_clipboard(&mut self, clipboard: &Arc<dyn Clipboard>) {
        self.clipboard.replace(Arc::clone(clipboard));
    }

    pub fn set_device_control_handler(&mut self, handler: Box<dyn DeviceControlHandler>) {
        self.device_control_handler.replace(handler);
    }

    pub fn set_notification_handler(&mut self, handler: Box<dyn AlertHandler>) {
        self.alert_handler.replace(handler);
    }

    /// Returns the title text associated with the terminal session.
    /// The title can be changed by the application using a number
    /// of escape sequences:
    /// OSC 2 is used to set the window title.
    /// OSC 1 is used to set the "icon title", which some terminal
    /// emulators interpret as a shorter title string for use when
    /// showing the tab title.
    /// Here in wezterm the terminalstate is isolated from other
    /// tabs; we process escape sequences without knowledge of other
    /// tabs, so we maintain both title strings here.
    /// The gui layer doesn't currently have a concept of what the
    /// overall window title should be beyond the title for the
    /// active tab with some decoration about the number of tabs.
    /// Shell toolkits such as oh-my-zsh prefer OSC 1 titles for
    /// abbreviated information.
    /// What we do here is prefer to return the OSC 1 icon title
    /// if it is set, otherwise return the OSC 2 window title.
    pub fn get_title(&self) -> &str {
        self.icon_title.as_ref().unwrap_or(&self.title)
    }

    /// Returns the current working directory associated with the
    /// terminal session.  The working directory can be changed by
    /// the applicaiton using the OSC 7 escape sequence.
    pub fn get_current_dir(&self) -> Option<&Url> {
        self.current_dir.as_ref()
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

    /// Returns a reference to the active screen (either the primary or
    /// the alternate screen).
    pub fn screen(&self) -> &Screen {
        &self.screen
    }

    /// Returns a mutable reference to the active screen (either the primary or
    /// the alternate screen).
    pub fn screen_mut(&mut self) -> &mut Screen {
        &mut self.screen
    }

    fn set_clipboard_contents(
        &self,
        selection: ClipboardSelection,
        text: Option<String>,
    ) -> anyhow::Result<()> {
        if let Some(clip) = self.clipboard.as_ref() {
            clip.set_contents(selection, text)?;
        }
        Ok(())
    }

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

    fn mouse_report_button_number(&self, event: &MouseEvent) -> i8 {
        let button = match event.button {
            MouseButton::None => self.current_mouse_button,
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

        code
    }

    fn mouse_wheel(&mut self, event: MouseEvent) -> Result<(), Error> {
        let button = self.mouse_report_button_number(&event);

        if self.sgr_mouse
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

    fn mouse_button_press(&mut self, event: MouseEvent) -> Result<(), Error> {
        self.current_mouse_button = event.button;

        if !(self.mouse_tracking || self.button_event_mouse || self.any_event_mouse) {
            return Ok(());
        }

        let button = self.mouse_report_button_number(&event);
        if self.sgr_mouse {
            write!(
                self.writer,
                "\x1b[<{};{};{}M",
                button,
                event.x + 1,
                event.y + 1
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

    fn mouse_button_release(&mut self, event: MouseEvent) -> Result<(), Error> {
        if self.current_mouse_button != MouseButton::None
            && (self.mouse_tracking || self.button_event_mouse || self.any_event_mouse)
        {
            if self.sgr_mouse {
                let release_button = self.mouse_report_button_number(&event);
                self.current_mouse_button = MouseButton::None;
                write!(
                    self.writer,
                    "\x1b[<{};{};{}m",
                    release_button,
                    event.x + 1,
                    event.y + 1
                )?;
                self.writer.flush()?;
            } else {
                let release_button = 3;
                self.current_mouse_button = MouseButton::None;
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

        Ok(())
    }

    fn mouse_move(&mut self, event: MouseEvent) -> Result<(), Error> {
        let reportable = self.any_event_mouse || self.current_mouse_button != MouseButton::None;
        // Note: self.mouse_tracking on its own is for clicks, not drags!
        if reportable && (self.button_event_mouse || self.any_event_mouse) {
            match self.last_mouse_move.as_ref() {
                Some(last) if *last == event => {
                    return Ok(());
                }
                _ => {}
            }
            self.last_mouse_move.replace(event.clone());

            let button = 32 + self.mouse_report_button_number(&event);

            if self.sgr_mouse {
                write!(
                    self.writer,
                    "\x1b[<{};{};{}M",
                    button,
                    event.x + 1,
                    event.y + 1
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
    pub fn mouse_event(&mut self, mut event: MouseEvent) -> Result<(), Error> {
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

    pub fn erase_scrollback_and_viewport(&mut self) {
        self.erase_in_display(EraseInDisplay::EraseScrollback);

        let row_index = self.screen.phys_row(self.cursor.y);
        let row = self.screen.lines[row_index].clone();

        self.erase_in_display(EraseInDisplay::EraseDisplay);

        self.screen.lines[0] = row;

        self.cursor.y = 0;
    }

    /// Discards the scrollback, leaving only the data that is present
    /// in the viewport.
    pub fn erase_scrollback(&mut self) {
        self.screen_mut().erase_scrollback();
    }

    /// Returns true if the associated application has enabled any of the
    /// supported mouse reporting modes.
    /// This is useful for the hosting GUI application to decide how best
    /// to dispatch mouse events to the terminal.
    pub fn is_mouse_grabbed(&self) -> bool {
        self.mouse_tracking || self.button_event_mouse || self.any_event_mouse
    }

    pub fn is_alt_screen_active(&self) -> bool {
        self.screen.is_alt_screen_active()
    }

    /// Returns true if the associated application has enabled
    /// bracketed paste mode, which can be helpful to the hosting
    /// GUI application to decide about fragmenting a large paste.
    pub fn bracketed_paste_enabled(&self) -> bool {
        self.bracketed_paste
    }

    /// Advise the terminal about a change in its focus state
    pub fn focus_changed(&mut self, focused: bool) {
        if !focused {
            self.current_mouse_button = MouseButton::None;
        }
        if self.focus_tracking {
            write!(self.writer, "{}{}", CSI, if focused { "I" } else { "O" }).ok();
            self.writer.flush().ok();
        }
    }

    /// Send text to the terminal that is the result of pasting.
    /// If bracketed paste mode is enabled, the paste is enclosed
    /// in the bracketing, otherwise it is fed to the writer as-is.
    pub fn send_paste(&mut self, text: &str) -> Result<(), Error> {
        let mut buf = String::new();
        if self.bracketed_paste {
            buf.push_str("\x1b[200~");
        }

        // This is a bit horrible; in general we try to stick with unix line
        // endings as the one-true representation because using canonical
        // CRLF can result is excess blank lines during a paste operation.
        // On Windows we're in a bit of a frustrating situation: pasting into
        // Windows console programs requires CRLF otherwise there is no newline
        // at all, but when in WSL, pasting with CRLF gives excess blank lines.
        //
        // To come to a compromise, if wezterm is running on Windows then we'll
        // use canonical CRLF unless the embedded application has enabled
        // bracketed paste: we can use bracketed paste mode as a signal that
        // the application will prefer newlines.
        //
        // In practice this means that unix shells and vim will get the
        // unix newlines in their pastes (which is the UX I want) and
        // cmd.exe will get CRLF.
        let canonicalize_line_endings = cfg!(windows) && !self.bracketed_paste;

        if canonicalize_line_endings {
            // Convert (\r|\n) -> \r\n, but not if it is \r\n anyway.
            let mut iter = text.chars();
            while let Some(c) = iter.next() {
                if c == '\n' {
                    buf.push_str("\r\n");
                } else if c == '\r' {
                    buf.push_str("\r\n");
                    match iter.next() {
                        Some(c) if c == '\n' => {
                            // Already emitted it above
                        }
                        Some(c) => buf.push(c),
                        None => {
                            // No more text and we already emitted \r\n above
                        }
                    }
                } else {
                    buf.push(c);
                }
            }
        } else {
            buf.push_str(text);
        }

        if self.bracketed_paste {
            buf.push_str("\x1b[201~");
        }

        self.writer.write_all(buf.as_bytes())?;
        self.writer.flush()?;
        Ok(())
    }

    fn csi_u_encode(&self, buf: &mut String, c: char, mods: KeyModifiers) -> Result<(), Error> {
        if self.config.enable_csi_u_key_encoding() {
            write!(buf, "\x1b[{};{}u", c as u32, 1 + encode_modifiers(mods))?;
        } else {
            let c = if mods.contains(KeyModifiers::CTRL) && ctrl_mapping(c).is_some() {
                ctrl_mapping(c).unwrap()
            } else {
                c
            };
            if mods.contains(KeyModifiers::ALT) {
                buf.push(0x1b as char);
            }
            write!(buf, "{}", c)?;
        }
        Ok(())
    }

    /// Processes a key_down event generated by the gui/render layer
    /// that is embedding the Terminal.  This method translates the
    /// keycode into a sequence of bytes to send to the slave end
    /// of the pty via the `Write`-able object provided by the caller.
    #[allow(clippy::cognitive_complexity)]
    pub fn key_down(&mut self, key: KeyCode, mods: KeyModifiers) -> Result<(), Error> {
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
            Char(c)
                if is_ambiguous_ascii_ctrl(c)
                    && mods.contains(KeyModifiers::CTRL)
                    && self.config.enable_csi_u_key_encoding() =>
            {
                self.csi_u_encode(&mut buf, c, mods)?;
                buf.as_str()
            }
            Char(c) if c.is_ascii_uppercase() && mods.contains(KeyModifiers::CTRL) => {
                self.csi_u_encode(&mut buf, c, mods)?;
                buf.as_str()
            }

            Char(c) if mods.contains(KeyModifiers::CTRL) && ctrl_mapping(c).is_some() => {
                let c = ctrl_mapping(c).unwrap();
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
                    self.csi_u_encode(&mut buf, c, mods)?;
                } else {
                    if mods.contains(KeyModifiers::ALT) {
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
                    self.csi_u_encode(&mut buf, c, mods)?;
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

                let csi_or_ss3 = if force_app
                    || (
                        self.application_cursor_keys
                        // Strict reading of DECCKM suggests that application_cursor_keys
                        // only applies when DECANM and DECKPAM are active, but that seems
                        // to break unmodified cursor keys in vim
                        /* && self.dec_ansi_mode && self.application_keypad */
                    ) {
                    // Use SS3 in application mode
                    SS3
                } else {
                    // otherwise use regular CSI
                    CSI
                };

                if mods.contains(KeyModifiers::SHIFT) || mods.contains(KeyModifiers::CTRL) {
                    write!(buf, "{}1;{}{}", CSI, 1 + encode_modifiers(mods), c)?;
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
                    let encoded_mods = encode_modifiers(mods);
                    if encoded_mods == 0 {
                        // If no modifiers are held, don't send the modifier
                        // sequence, as the modifier encoding is a CSI-u extension.
                        write!(buf, "{}~", intro)?;
                    } else {
                        write!(buf, "{};{}~", intro, 1 + encoded_mods)?;
                    }
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
        self.writer.write_all(to_send.as_bytes())?;
        self.writer.flush()?;

        Ok(())
    }

    /// Informs the terminal that the viewport of the window has resized to the
    /// specified dimensions.
    pub fn resize(
        &mut self,
        physical_rows: usize,
        physical_cols: usize,
        pixel_width: usize,
        pixel_height: usize,
    ) {
        let adjusted_cursor = self
            .screen
            .resize(physical_rows, physical_cols, self.cursor);
        self.top_and_bottom_margins = 0..physical_rows as i64;
        self.left_and_right_margins = 0..physical_cols;
        self.pixel_height = pixel_height;
        self.pixel_width = pixel_width;
        self.tabs.resize(physical_cols);
        self.set_cursor_pos(
            &Position::Absolute(adjusted_cursor.x as i64),
            &Position::Absolute(adjusted_cursor.y),
        );
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
            shape: self.cursor.shape,
            visibility: if self.cursor_visible {
                CursorVisibility::Visible
            } else {
                CursorVisibility::Hidden
            },
        }
    }

    pub fn user_vars(&self) -> &HashMap<String, String> {
        &self.user_vars
    }

    /// Sets the cursor position to precisely the x and values provided
    fn set_cursor_position_absolute(&mut self, x: usize, y: VisibleRowIndex) {
        let old_y = self.cursor.y;

        self.cursor.y = y;
        self.cursor.x = x;
        self.wrap_next = false;

        let screen = self.screen_mut();
        screen.dirty_line(old_y);
        screen.dirty_line(y);
    }

    /// Sets the cursor position. x and y are 0-based and relative to the
    /// top left of the visible screen.
    fn set_cursor_pos(&mut self, x: &Position, y: &Position) {
        let x = match *x {
            Position::Relative(x) => (self.cursor.x as i64 + x)
                .min(
                    if self.dec_origin_mode {
                        self.left_and_right_margins.end
                    } else {
                        self.screen().physical_cols
                    } as i64
                        - 1,
                )
                .max(0),
            Position::Absolute(x) => (x + if self.dec_origin_mode {
                self.left_and_right_margins.start
            } else {
                0
            } as i64)
                .min(
                    if self.dec_origin_mode {
                        self.left_and_right_margins.end
                    } else {
                        self.screen().physical_cols
                    } as i64
                        - 1,
                )
                .max(0),
        };

        let y = match *y {
            Position::Relative(y) => (self.cursor.y + y)
                .min(
                    if self.dec_origin_mode {
                        self.top_and_bottom_margins.end
                    } else {
                        self.screen().physical_rows as i64
                    } - 1,
                )
                .max(0),
            Position::Absolute(y) => (y + if self.dec_origin_mode {
                self.top_and_bottom_margins.start
            } else {
                0
            })
            .min(
                if self.dec_origin_mode {
                    self.top_and_bottom_margins.end
                } else {
                    self.screen().physical_rows as i64
                } - 1,
            )
            .max(0),
        };

        self.set_cursor_position_absolute(x as usize, y);
    }

    fn scroll_up(&mut self, num_rows: usize) {
        let top_and_bottom_margins = self.top_and_bottom_margins.clone();
        let left_and_right_margins = self.left_and_right_margins.clone();
        self.screen_mut().scroll_up_within_margins(
            &top_and_bottom_margins,
            &left_and_right_margins,
            num_rows,
        )
    }

    fn scroll_down(&mut self, num_rows: usize) {
        let top_and_bottom_margins = self.top_and_bottom_margins.clone();
        let left_and_right_margins = self.left_and_right_margins.clone();
        self.screen_mut().scroll_down_within_margins(
            &top_and_bottom_margins,
            &left_and_right_margins,
            num_rows,
        )
    }

    /// Defined by FinalTermSemanticPrompt; a fresh-line is a NOP if the
    /// cursor is already at the left margin, otherwise it is the same as
    /// a new line.
    fn fresh_line(&mut self) {
        if self.cursor.x == self.left_and_right_margins.start {
            return;
        }
        self.new_line(true);
    }

    fn new_line(&mut self, move_to_first_column: bool) {
        let x = if move_to_first_column {
            self.left_and_right_margins.start
        } else {
            self.cursor.x
        };
        let y = self.cursor.y;
        let y = if y == self.top_and_bottom_margins.end - 1 {
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
        if self.left_and_right_margins.contains(&self.cursor.x) {
            if self.cursor.y == self.top_and_bottom_margins.end - 1 {
                self.scroll_up(1);
            } else {
                self.set_cursor_pos(&Position::Relative(0), &Position::Relative(1));
            }
        }
    }

    /// Moves the cursor to the first position on the next line.
    /// If the cursor is at the bottom margin, the page scrolls up.
    fn c1_nel(&mut self) {
        let y_clamp = if self.top_and_bottom_margins.contains(&self.cursor.y) {
            self.top_and_bottom_margins.end - 1
        } else {
            self.screen().physical_rows as VisibleRowIndex - 1
        };

        if self.left_and_right_margins.contains(&self.cursor.x) {
            if self.cursor.y == self.top_and_bottom_margins.end - 1 {
                self.scroll_up(1);
                self.set_cursor_position_absolute(self.left_and_right_margins.start, self.cursor.y);
            } else {
                self.set_cursor_position_absolute(
                    self.left_and_right_margins.start,
                    (self.cursor.y + 1).min(y_clamp),
                );
            }
        } else {
            // When outside left/right margins, NEL moves but does not scroll
            self.set_cursor_position_absolute(
                if self.cursor.x < self.left_and_right_margins.start {
                    self.cursor.x
                } else {
                    self.left_and_right_margins.start
                },
                (self.cursor.y + 1).min(y_clamp),
            );
        }
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
            None => self.left_and_right_margins.end - 1,
        };
        self.cursor.x = x.min(self.left_and_right_margins.end - 1);
        let y = self.cursor.y;
        self.screen_mut().dirty_line(y);
    }

    /// Move the cursor up 1 line.  If the position is at the top scroll margin,
    /// scroll the region down.
    fn c1_reverse_index(&mut self) {
        if self.left_and_right_margins.contains(&self.cursor.x) {
            if self.cursor.y == self.top_and_bottom_margins.start {
                self.scroll_down(1);
            } else {
                self.set_cursor_pos(&Position::Relative(0), &Position::Relative(-1));
            }
        }
    }

    fn set_hyperlink(&mut self, link: Option<Hyperlink>) {
        self.pen.set_hyperlink(match link {
            Some(hyperlink) => Some(Arc::new(hyperlink)),
            None => None,
        });
    }

    fn sixel(&mut self, sixel: Box<Sixel>) {
        let (width, height) = sixel.dimensions();

        let mut private_color_map;
        let color_map = if self.use_private_color_registers_for_each_graphic {
            private_color_map = default_color_map();
            &mut private_color_map
        } else {
            &mut self.color_map
        };

        let mut image = if sixel.background_is_transparent {
            image::RgbaImage::new(width, height)
        } else {
            let background_color = color_map
                .get(&0)
                .cloned()
                .unwrap_or(RgbColor::new_8bpc(0, 0, 0));
            let (red, green, blue) = background_color.to_tuple_rgb8();
            image::RgbaImage::from_pixel(width, height, [red, green, blue, 0xffu8].into())
        };

        let mut x = 0;
        let mut y = 0;
        let mut foreground_color = RgbColor::new_8bpc(0, 0xff, 0);

        let mut emit_sixel = |d: &u8, foreground_color: &RgbColor, x: u32, y: u32| {
            let (red, green, blue) = foreground_color.to_tuple_rgb8();
            for bitno in 0..6 {
                if y + bitno >= height {
                    break;
                }
                let on = (d & (1 << bitno)) != 0;
                if on {
                    image.get_pixel_mut(x, y + bitno).0 = [red, green, blue, 0xffu8];
                }
            }
        };

        for d in &sixel.data {
            match d {
                SixelData::Data(d) => {
                    emit_sixel(d, &foreground_color, x, y);
                    x += 1;
                }

                SixelData::Repeat { repeat_count, data } => {
                    for _ in 0..*repeat_count {
                        emit_sixel(data, &foreground_color, x, y);
                        x += 1;
                    }
                }

                SixelData::CarriageReturn => x = 0,
                SixelData::NewLine => {
                    x = 0;
                    y += 6;
                }

                SixelData::DefineColorMapRGB { color_number, rgb } => {
                    color_map.insert(*color_number, *rgb);
                }

                SixelData::DefineColorMapHSL {
                    color_number,
                    hue_angle,
                    saturation,
                    lightness,
                } => {
                    use palette::encoding::pixel::Pixel;
                    // Sixel's hue angles are: blue=0, red=120, green=240,
                    // whereas Hsl has red=0, green=120, blue=240.
                    // Looking at red, we need to rotate left by 120 to
                    // go from sixel red to palette::RgbHue red.
                    // Negative values wrap around the circle.
                    // https://github.com/wez/wezterm/issues/775
                    let angle = (*hue_angle as f32) - 120.0;
                    let angle = if angle < 0. { 360.0 + angle } else { angle };
                    let hue = palette::RgbHue::from_degrees(angle);
                    let hsl =
                        palette::Hsl::new(hue, *saturation as f32 / 100., *lightness as f32 / 100.);
                    let rgb: palette::Srgb = hsl.into();
                    let rgb: [u8; 3] = rgb.into_linear().into_format().into_raw();

                    color_map.insert(*color_number, RgbColor::new_8bpc(rgb[0], rgb[1], rgb[2]));
                }

                SixelData::SelectColorMapEntry(n) => {
                    foreground_color = color_map.get(n).cloned().unwrap_or_else(|| {
                        log::error!("sixel selected noexistent colormap entry {}", n);
                        RgbColor::new_8bpc(255, 255, 255)
                    });
                }
            }
        }

        let mut png_image_data = Vec::new();
        let encoder = image::png::PngEncoder::new(&mut png_image_data);
        if let Err(e) = encoder.encode(&image.into_vec(), width, height, image::ColorType::Rgba8) {
            error!("failed to encode sixel data into png: {}", e);
            return;
        }

        let image_data = self.raw_image_to_image_data(png_image_data.into_boxed_slice());
        self.assign_image_to_cells(width, height, image_data, false);
    }

    /// cache recent images and avoid assigning a new id for repeated data!
    fn raw_image_to_image_data(&mut self, raw_data: Box<[u8]>) -> Arc<ImageData> {
        use sha2::Digest;
        let mut hasher = sha2::Sha256::new();
        hasher.update(&raw_data);
        let key = hasher.finalize().into();

        if let Some(item) = self.image_cache.get(&key) {
            Arc::clone(item)
        } else {
            let image_data = Arc::new(ImageData::with_raw_data(raw_data));
            self.image_cache.put(key, Arc::clone(&image_data));
            image_data
        }
    }

    fn assign_image_to_cells(
        &mut self,
        width: u32,
        height: u32,
        image_data: Arc<ImageData>,
        iterm_cursor_position: bool,
    ) {
        let physical_cols = self.screen().physical_cols;
        let physical_rows = self.screen().physical_rows;
        let cell_pixel_width = self.pixel_width / physical_cols;
        let cell_pixel_height = self.pixel_height / physical_rows;

        let width_in_cells = (width as f32 / cell_pixel_width as f32).ceil() as usize;
        let height_in_cells = (height as f32 / cell_pixel_height as f32).ceil() as usize;

        let mut ypos = NotNan::new(0.0).unwrap();
        let cursor_x = self.cursor.x;
        let x_delta = 1.0 / (width as f32 / (self.pixel_width as f32 / physical_cols as f32));
        let y_delta = 1.0 / (height as f32 / (self.pixel_height as f32 / physical_rows as f32));
        log::debug!(
            "image is {}x{} cells, {}x{} pixels, x_delta:{} y_delta:{} ({}x{}@{}x{})",
            width_in_cells,
            height_in_cells,
            width,
            height,
            x_delta,
            y_delta,
            physical_cols,
            physical_rows,
            self.pixel_width,
            self.pixel_height
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
                let mut cell = self
                    .screen()
                    .get_cell(cursor_x + x, cursor_y)
                    .cloned()
                    .unwrap_or_else(|| Cell::new(' ', CellAttributes::default()));
                cell.attrs_mut().set_image(Some(Box::new(ImageCell::new(
                    TextureCoordinate::new(xpos, ypos),
                    TextureCoordinate::new(xpos + x_delta, ypos + y_delta),
                    image_data.clone(),
                ))));

                self.screen_mut().set_cell(cursor_x + x, cursor_y, &cell);
                xpos += x_delta;
            }
            ypos += y_delta;
            self.new_line(false);
        }

        // Sixel places the cursor under the left corner of the image,
        // unless sixel_scrolls_right is enabled.
        // iTerm places it after the bottom right corner.
        if iterm_cursor_position || self.sixel_scrolls_right {
            self.set_cursor_pos(
                &Position::Relative(width_in_cells as i64),
                &Position::Relative(-1),
            );
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

        struct Info {
            width: u32,
            height: u32,
            format: ImageFormat,
        }

        fn dimensions(data: &[u8]) -> anyhow::Result<Info> {
            let reader =
                image::io::Reader::new(std::io::Cursor::new(data)).with_guessed_format()?;
            let format = reader
                .format()
                .ok_or_else(|| anyhow::anyhow!("unknown format!?"))?;
            let (width, height) = reader.into_dimensions()?;
            Ok(Info {
                width,
                height,
                format,
            })
        }

        let info = match dimensions(&image.data) {
            Ok(dims) => dims,
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
        let aspect = info.width as f32 / info.height as f32;

        let (width, height) = match (width, height) {
            (None, None) => {
                // Take the image's native size
                let width = info.width as usize;
                let height = info.height as usize;
                // but ensure that it fits
                if width as usize > self.pixel_width || height as usize > self.pixel_height {
                    let width = width as f32;
                    let height = height as f32;
                    let mut candidates = vec![];

                    let x_scale = self.pixel_width as f32 / width;
                    if height * x_scale <= self.pixel_height as f32 {
                        candidates.push((self.pixel_width, (height * x_scale) as usize));
                    }
                    let y_scale = self.pixel_height as f32 / height;
                    if width * y_scale <= self.pixel_width as f32 {
                        candidates.push(((width * y_scale) as usize, self.pixel_height));
                    }

                    candidates.sort_by(|a, b| (a.0 * a.1).cmp(&(b.0 * b.1)));

                    candidates.pop().unwrap()
                } else {
                    (width, height)
                }
            }
            (Some(w), None) => {
                let h = w as f32 / aspect;
                (w, h as usize)
            }
            (None, Some(h)) => {
                let w = h as f32 * aspect;
                (w as usize, h)
            }
            (Some(w), Some(_)) if image.preserve_aspect_ratio => {
                let h = w as f32 / aspect;
                (w, h as usize)
            }
            (Some(w), Some(h)) => (w, h),
        };

        let downscaled = (width < info.width as usize) || (height < info.height as usize);
        let data = match (downscaled, info.format) {
            (true, ImageFormat::Gif) | (true, ImageFormat::Png) | (false, _) => {
                // Don't resample things that might be animations,
                // or things that don't need resampling
                image.data
            }
            (true, _) => match image::load_from_memory(&image.data) {
                Ok(im) => {
                    let im = im.resize_exact(width as u32, height as u32, FilterType::CatmullRom);
                    let mut data = vec![];
                    match im.write_to(&mut data, ImageFormat::Png) {
                        Ok(_) => data.into_boxed_slice(),
                        Err(_) => image.data,
                    }
                }
                Err(_) => image.data,
            },
        };

        let image_data = self.raw_image_to_image_data(data);
        self.assign_image_to_cells(width as u32, height as u32, image_data, true);
    }

    fn perform_device(&mut self, dev: Device) {
        match dev {
            Device::DeviceAttributes(a) => log::warn!("unhandled: {:?}", a),
            Device::SoftReset => {
                // TODO: see https://vt100.net/docs/vt510-rm/DECSTR.html
                self.pen = CellAttributes::default();
                self.insert = false;
                self.dec_origin_mode = false;
                // Note that xterm deviates from the documented DECSTR
                // setting for dec_auto_wrap, so we do too
                self.dec_auto_wrap = true;
                self.application_cursor_keys = false;
                self.application_keypad = false;
                self.top_and_bottom_margins = 0..self.screen().physical_rows as i64;
                self.left_and_right_margins = 0..self.screen().physical_cols;
                self.screen.activate_alt_screen();
                self.screen.saved_cursor().take();
                self.screen.activate_primary_screen();
                self.screen.saved_cursor().take();

                self.reverse_wraparound_mode = false;
            }
            Device::RequestPrimaryDeviceAttributes => {
                let mut ident = "\x1b[?65".to_string(); // Vt500
                ident.push_str(";4"); // Sixel graphics
                ident.push_str(";6"); // Selective erase
                ident.push_str(";18"); // windowing extensions
                ident.push_str(";22"); // ANSI color, vt525
                ident.push('c');

                self.writer.write(ident.as_bytes()).ok();
                self.writer.flush().ok();
            }
            Device::RequestSecondaryDeviceAttributes => {
                self.writer.write(b"\x1b[>0;0;0c").ok();
                self.writer.flush().ok();
            }
            Device::RequestTertiaryDeviceAttributes => {
                self.writer.write(b"\x1b[=00000000").ok();
                self.writer.write(ST.as_bytes()).ok();
                self.writer.flush().ok();
            }
            Device::RequestTerminalNameAndVersion => {
                self.writer.write(DCS.as_bytes()).ok();
                self.writer
                    .write(format!(">|{} {}", self.term_program, self.term_version).as_bytes())
                    .ok();
                self.writer.write(ST.as_bytes()).ok();
                self.writer.flush().ok();
            }
            Device::StatusReport => {
                self.writer.write(b"\x1b[0n").ok();
                self.writer.flush().ok();
            }
            Device::XtSmGraphics(g) => {
                let response = if matches!(g.item, XtSmGraphicsItem::Unspecified(_)) {
                    XtSmGraphics {
                        item: g.item,
                        action_or_status: XtSmGraphicsStatus::InvalidItem.to_i64(),
                        value: vec![],
                    }
                } else {
                    match g.action() {
                        None | Some(XtSmGraphicsAction::SetToValue) => XtSmGraphics {
                            item: g.item,
                            action_or_status: XtSmGraphicsStatus::InvalidAction.to_i64(),
                            value: vec![],
                        },
                        Some(XtSmGraphicsAction::ResetToDefault) => XtSmGraphics {
                            item: g.item,
                            action_or_status: XtSmGraphicsStatus::Success.to_i64(),
                            value: vec![],
                        },
                        Some(XtSmGraphicsAction::ReadMaximumAllowedValue)
                        | Some(XtSmGraphicsAction::ReadAttribute) => match g.item {
                            XtSmGraphicsItem::Unspecified(_) => unreachable!("checked above"),
                            XtSmGraphicsItem::NumberOfColorRegisters => XtSmGraphics {
                                item: g.item,
                                action_or_status: XtSmGraphicsStatus::Success.to_i64(),
                                value: vec![65536],
                            },
                            XtSmGraphicsItem::RegisGraphicsGeometry
                            | XtSmGraphicsItem::SixelGraphicsGeometry => XtSmGraphics {
                                item: g.item,
                                action_or_status: XtSmGraphicsStatus::Success.to_i64(),
                                value: vec![self.pixel_width as i64, self.pixel_height as i64],
                            },
                        },
                    }
                };

                let dev = Device::XtSmGraphics(response);

                write!(self.writer, "\x1b[{}", dev).ok();
                self.writer.flush().ok();
            }
        }
    }

    fn decqrm_response(&mut self, mode: Mode, mut recognized: bool, enabled: bool) {
        let (is_dec, number) = match &mode {
            Mode::QueryDecPrivateMode(DecPrivateMode::Code(code)) => (true, code.to_u16().unwrap()),
            Mode::QueryDecPrivateMode(DecPrivateMode::Unspecified(code)) => {
                recognized = false;
                (true, *code)
            }
            Mode::QueryMode(TerminalMode::Code(code)) => (false, code.to_u16().unwrap()),
            Mode::QueryMode(TerminalMode::Unspecified(code)) => {
                recognized = false;
                (false, *code)
            }
            _ => unreachable!(),
        };

        let prefix = if is_dec { "?" } else { "" };

        let status = if recognized {
            if enabled {
                1 // set
            } else {
                2 // reset
            }
        } else {
            0
        };

        log::trace!("{:?} -> recognized={} status={}", mode, recognized, status);
        write!(self.writer, "\x1b[{}{};{}$y", prefix, number, status).ok();
    }

    fn perform_csi_mode(&mut self, mode: Mode) {
        match mode {
            Mode::SetDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::StartBlinkingCursor,
            ))
            | Mode::ResetDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::StartBlinkingCursor,
            )) => {}
            Mode::QueryDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::StartBlinkingCursor,
            )) => {
                self.decqrm_response(mode, true, false);
            }

            Mode::SetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::AutoRepeat))
            | Mode::ResetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::AutoRepeat)) => {
                // We leave key repeat to the GUI layer prefs
            }

            Mode::SetDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::ReverseWraparound,
            )) => {
                self.reverse_wraparound_mode = true;
            }

            Mode::ResetDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::ReverseWraparound,
            )) => {
                self.reverse_wraparound_mode = false;
            }

            Mode::QueryDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::ReverseWraparound,
            )) => {
                self.decqrm_response(mode, true, self.reverse_wraparound_mode);
            }

            Mode::SetDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::LeftRightMarginMode,
            )) => {
                self.left_and_right_margin_mode = true;
            }

            Mode::ResetDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::LeftRightMarginMode,
            )) => {
                self.left_and_right_margin_mode = false;
                self.left_and_right_margins = 0..self.screen().physical_cols;
            }

            Mode::QueryDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::LeftRightMarginMode,
            )) => {
                self.decqrm_response(mode, true, self.left_and_right_margin_mode);
            }

            Mode::SetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::SaveCursor)) => {
                self.dec_save_cursor();
            }

            Mode::ResetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::SaveCursor)) => {
                self.dec_restore_cursor();
            }

            Mode::SetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::AutoWrap)) => {
                self.dec_auto_wrap = true;
            }

            Mode::ResetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::AutoWrap)) => {
                self.dec_auto_wrap = false;
            }

            Mode::QueryDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::AutoWrap)) => {
                self.decqrm_response(mode, true, self.dec_auto_wrap);
            }

            Mode::SetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::OriginMode)) => {
                self.dec_origin_mode = true;
                self.set_cursor_pos(&Position::Absolute(0), &Position::Absolute(0));
            }

            Mode::ResetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::OriginMode)) => {
                self.dec_origin_mode = false;
                self.set_cursor_pos(&Position::Absolute(0), &Position::Absolute(0));
            }

            Mode::QueryDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::OriginMode)) => {
                self.decqrm_response(mode, true, self.dec_origin_mode);
            }

            Mode::SetDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::UsePrivateColorRegistersForEachGraphic,
            )) => {
                self.use_private_color_registers_for_each_graphic = true;
            }
            Mode::ResetDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::UsePrivateColorRegistersForEachGraphic,
            )) => {
                self.use_private_color_registers_for_each_graphic = false;
            }
            Mode::QueryDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::UsePrivateColorRegistersForEachGraphic,
            )) => {
                self.decqrm_response(
                    mode,
                    true,
                    self.use_private_color_registers_for_each_graphic,
                );
            }

            Mode::SetDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::SynchronizedOutput,
            )) => {
                // This is handled in wezterm's mux
            }
            Mode::ResetDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::SynchronizedOutput,
            )) => {
                // This is handled in wezterm's mux
            }
            Mode::QueryDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::SynchronizedOutput,
            )) => {
                // This is handled in wezterm's mux; if we get here, then it isn't enabled,
                // so we always report false
                self.decqrm_response(mode, true, false);
            }

            Mode::SetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::SmoothScroll))
            | Mode::ResetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::SmoothScroll)) => {
                // We always output at our "best" rate
            }

            Mode::SetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::ReverseVideo))
            | Mode::ResetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::ReverseVideo)) => {
                // I'm mostly intentionally ignoring this in favor
                // of respecting the configured colors
            }

            Mode::SetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::Select132Columns))
            | Mode::ResetDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::Select132Columns,
            )) => {
                // Note: we don't support 132 column mode so we treat
                // both set/reset as the same and we're really just here
                // for the other side effects of this sequence
                // https://vt100.net/docs/vt510-rm/DECCOLM.html

                self.top_and_bottom_margins = 0..self.screen().physical_rows as i64;
                self.left_and_right_margins = 0..self.screen().physical_cols;
                self.set_cursor_pos(&Position::Absolute(0), &Position::Absolute(0));
                self.erase_in_display(EraseInDisplay::EraseDisplay);
            }

            Mode::SetMode(TerminalMode::Code(TerminalModeCode::Insert)) => {
                self.insert = true;
            }
            Mode::ResetMode(TerminalMode::Code(TerminalModeCode::Insert)) => {
                self.insert = false;
            }
            Mode::QueryMode(TerminalMode::Code(TerminalModeCode::Insert)) => {
                self.decqrm_response(mode, true, self.insert);
            }

            Mode::SetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::BracketedPaste)) => {
                self.bracketed_paste = true;
            }
            Mode::ResetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::BracketedPaste)) => {
                self.bracketed_paste = false;
            }
            Mode::QueryDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::BracketedPaste)) => {
                self.decqrm_response(mode, true, self.bracketed_paste);
            }

            Mode::SetDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::OptEnableAlternateScreen,
            ))
            | Mode::SetDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::EnableAlternateScreen,
            )) => {
                if !self.screen.is_alt_screen_active() {
                    self.screen.activate_alt_screen();
                    self.pen = CellAttributes::default();
                }
            }
            Mode::ResetDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::OptEnableAlternateScreen,
            )) => {
                if self.screen.is_alt_screen_active() {
                    self.pen = CellAttributes::default();
                    self.erase_in_display(EraseInDisplay::EraseDisplay);
                    self.screen.activate_primary_screen();
                }
            }

            Mode::ResetDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::EnableAlternateScreen,
            )) => {
                if self.screen.is_alt_screen_active() {
                    self.screen.activate_primary_screen();
                    self.pen = CellAttributes::default();
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
            Mode::QueryDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::ApplicationCursorKeys,
            )) => {
                self.decqrm_response(mode, true, self.application_cursor_keys);
            }

            Mode::SetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::SixelScrolling)) => {
                self.sixel_scrolling = true;
            }
            Mode::ResetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::SixelScrolling)) => {
                self.sixel_scrolling = false;
            }
            Mode::QueryDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::SixelScrolling)) => {
                self.decqrm_response(mode, true, self.sixel_scrolling);
            }

            Mode::SetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::DecAnsiMode)) => {
                self.dec_ansi_mode = true;
            }
            Mode::ResetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::DecAnsiMode)) => {
                self.dec_ansi_mode = false;
            }
            Mode::QueryDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::DecAnsiMode)) => {
                self.decqrm_response(mode, true, self.dec_ansi_mode);
            }

            Mode::SetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::ShowCursor)) => {
                self.cursor_visible = true;
            }
            Mode::ResetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::ShowCursor)) => {
                self.cursor_visible = false;
            }
            Mode::QueryDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::ShowCursor)) => {
                self.decqrm_response(mode, true, self.cursor_visible);
            }
            Mode::SetMode(TerminalMode::Code(TerminalModeCode::ShowCursor)) => {
                self.cursor_visible = true;
            }
            Mode::ResetMode(TerminalMode::Code(TerminalModeCode::ShowCursor)) => {
                self.cursor_visible = false;
            }

            Mode::SetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::MouseTracking)) => {
                self.mouse_tracking = true;
                self.last_mouse_move.take();
            }
            Mode::ResetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::MouseTracking)) => {
                self.mouse_tracking = false;
                self.last_mouse_move.take();
            }
            Mode::QueryDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::MouseTracking)) => {
                self.decqrm_response(mode, true, self.mouse_tracking);
            }

            Mode::SetDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::HighlightMouseTracking,
            ))
            | Mode::ResetDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::HighlightMouseTracking,
            )) => {}

            Mode::SetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::ButtonEventMouse)) => {
                self.button_event_mouse = true;
                self.last_mouse_move.take();
            }
            Mode::ResetDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::ButtonEventMouse,
            )) => {
                self.button_event_mouse = false;
                self.last_mouse_move.take();
            }
            Mode::QueryDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::ButtonEventMouse,
            )) => {
                self.decqrm_response(mode, true, self.button_event_mouse);
            }

            Mode::SetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::AnyEventMouse)) => {
                self.any_event_mouse = true;
                self.last_mouse_move.take();
            }
            Mode::ResetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::AnyEventMouse)) => {
                self.any_event_mouse = false;
                self.last_mouse_move.take();
            }
            Mode::QueryDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::AnyEventMouse)) => {
                self.decqrm_response(mode, true, self.any_event_mouse);
            }

            Mode::SetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::FocusTracking)) => {
                self.focus_tracking = true;
                self.last_mouse_move.take();
            }
            Mode::ResetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::FocusTracking)) => {
                self.focus_tracking = false;
                self.last_mouse_move.take();
            }
            Mode::QueryDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::FocusTracking)) => {
                self.decqrm_response(mode, true, self.focus_tracking);
            }

            Mode::SetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::SGRMouse)) => {
                self.sgr_mouse = true;
                self.last_mouse_move.take();
            }
            Mode::ResetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::SGRMouse)) => {
                self.sgr_mouse = false;
                self.last_mouse_move.take();
            }
            Mode::QueryDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::SGRMouse)) => {
                self.decqrm_response(mode, true, self.sgr_mouse);
            }

            Mode::SetDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::SixelScrollsRight,
            )) => {
                self.sixel_scrolls_right = true;
            }
            Mode::ResetDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::SixelScrollsRight,
            )) => {
                self.sixel_scrolls_right = false;
            }
            Mode::QueryDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::SixelScrollsRight,
            )) => {
                self.decqrm_response(mode, true, self.sixel_scrolls_right);
            }

            Mode::SetDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::ClearAndEnableAlternateScreen,
            )) => {
                if !self.screen.is_alt_screen_active() {
                    self.dec_save_cursor();
                    self.screen.activate_alt_screen();
                    self.set_cursor_pos(&Position::Absolute(0), &Position::Absolute(0));
                    self.pen = CellAttributes::default();
                    self.erase_in_display(EraseInDisplay::EraseDisplay);
                }
            }
            Mode::ResetDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::ClearAndEnableAlternateScreen,
            )) => {
                if self.screen.is_alt_screen_active() {
                    self.screen.activate_primary_screen();
                    self.dec_restore_cursor();
                }
            }
            Mode::SaveDecPrivateMode(DecPrivateMode::Code(n))
            | Mode::RestoreDecPrivateMode(DecPrivateMode::Code(n)) => {
                log::warn!("save/restore dec mode {:?} unimplemented", n)
            }

            Mode::SetDecPrivateMode(DecPrivateMode::Unspecified(_))
            | Mode::ResetDecPrivateMode(DecPrivateMode::Unspecified(_))
            | Mode::SaveDecPrivateMode(DecPrivateMode::Unspecified(_))
            | Mode::RestoreDecPrivateMode(DecPrivateMode::Unspecified(_)) => {
                log::warn!("unhandled DecPrivateMode {:?}", mode);
            }

            mode @ Mode::SetMode(_) | mode @ Mode::ResetMode(_) => {
                log::warn!("unhandled {:?}", mode);
            }

            Mode::XtermKeyMode { resource, value } => {
                log::warn!("unhandled XtermKeyMode {:?} {:?}", resource, value);
            }

            Mode::QueryDecPrivateMode(_) | Mode::QueryMode(_) => {
                self.decqrm_response(mode, false, false);
            }
        }
    }

    fn checksum_rectangle(&mut self, left: u32, top: u32, right: u32, bottom: u32) -> u16 {
        let y_origin = if self.dec_origin_mode {
            self.top_and_bottom_margins.start
        } else {
            0
        } as u32;
        let x_origin = if self.dec_origin_mode {
            self.left_and_right_margins.start
        } else {
            0
        };
        let screen = self.screen_mut();
        let mut checksum = 0;
        /*
        debug!(
            "checksum left={} top={} right={} bottom={}",
            left as usize + x_origin,
            top + y_origin,
            right as usize + x_origin,
            bottom + y_origin
        );
        */

        for y in top..=bottom {
            let line_idx = screen.phys_row(VisibleRowIndex::from(y_origin + y));
            let line = screen.line_mut(line_idx);
            for (col, cell) in line
                .cells()
                .iter()
                .enumerate()
                .skip(x_origin + left as usize)
            {
                if col > x_origin + right as usize {
                    break;
                }

                let ch = cell.str().chars().nth(0).unwrap() as u32;
                // debug!("y={} col={} ch={:x} cell={:?}", y + y_origin, col, ch, cell);

                checksum += u16::from(ch as u8);
            }
        }
        checksum
    }

    fn perform_csi_window(&mut self, window: Window) {
        match window {
            Window::ReportTextAreaSizeCells => {
                let screen = self.screen();
                let height = Some(screen.physical_rows as i64);
                let width = Some(screen.physical_cols as i64);

                let response = Window::ResizeWindowCells { width, height };
                write!(self.writer, "{}", CSI::Window(response)).ok();
                self.writer.flush().ok();
            }

            Window::ReportCellSizePixels => {
                let screen = self.screen();
                let height = screen.physical_rows;
                let width = screen.physical_cols;
                let response = Window::ReportCellSizePixelsResponse {
                    width: Some((self.pixel_width / width) as i64),
                    height: Some((self.pixel_height / height) as i64),
                };
                write!(self.writer, "{}", CSI::Window(response)).ok();
                self.writer.flush().ok();
            }

            Window::ReportTextAreaSizePixels => {
                let response = Window::ResizeWindowPixels {
                    width: Some(self.pixel_width as i64),
                    height: Some(self.pixel_height as i64),
                };
                write!(self.writer, "{}", CSI::Window(response)).ok();
                self.writer.flush().ok();
            }

            Window::ReportWindowTitle => {
                write!(
                    self.writer,
                    "{}",
                    OperatingSystemCommand::SetWindowTitleSun(self.title.clone())
                )
                .ok();
                self.writer.flush().ok();
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
                write!(self.writer, "\x1bP{}!~{:04x}\x1b\\", request_id, checksum).ok();
                self.writer.flush().ok();
            }
            Window::ResizeWindowCells { .. } => {
                // We don't allow the application to change the window size; that's
                // up to the user!
            }
            Window::Iconify | Window::DeIconify => {}
            Window::PopIconAndWindowTitle
            | Window::PopWindowTitle
            | Window::PopIconTitle
            | Window::PushIconAndWindowTitle
            | Window::PushIconTitle
            | Window::PushWindowTitle => {}

            _ => log::warn!("unhandled Window CSI {:?}", window),
        }
    }

    fn erase_in_display(&mut self, erase: EraseInDisplay) {
        let cy = self.cursor.y;
        let pen = self.pen.clone_sgr_only();
        let rows = self.screen().physical_rows as VisibleRowIndex;
        let col_range = 0..self.screen().physical_cols;
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
                self.screen_mut().erase_scrollback();
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

                if x >= self.left_and_right_margins.start && x < self.left_and_right_margins.end {
                    let right_margin = self.left_and_right_margins.end;
                    let limit = (x + n as usize).min(right_margin);

                    let screen = self.screen_mut();
                    for _ in x..limit as usize {
                        screen.erase_cell(x, y, right_margin);
                    }
                }
            }
            Edit::DeleteLine(n) => {
                if self.top_and_bottom_margins.contains(&self.cursor.y)
                    && self.left_and_right_margins.contains(&self.cursor.x)
                {
                    let top_and_bottom_margins = self.cursor.y..self.top_and_bottom_margins.end;
                    let left_and_right_margins = self.left_and_right_margins.clone();
                    self.screen_mut().scroll_up_within_margins(
                        &top_and_bottom_margins,
                        &left_and_right_margins,
                        n as usize,
                    );
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
                    EraseInLine::EraseToStartOfLine => 0..cx + 1,
                    EraseInLine::EraseLine => 0..cols,
                };

                self.screen_mut().clear_line(cy, range.clone(), &pen);
            }
            Edit::InsertCharacter(n) => {
                // https://vt100.net/docs/vt510-rm/ICH.html
                // The ICH sequence inserts Pn blank characters with the normal character
                // attribute. The cursor remains at the beginning of the blank characters. Text
                // between the cursor and right margin moves to the right. Characters scrolled past
                // the right margin are lost. ICH has no effect outside the scrolling margins.

                let y = self.cursor.y;
                let x = self.cursor.x;
                if self.top_and_bottom_margins.contains(&y)
                    && self.left_and_right_margins.contains(&x)
                {
                    let margin = self.left_and_right_margins.end;
                    let screen = self.screen_mut();
                    for _ in 0..n as usize {
                        screen.insert_cell(x, y, margin);
                    }
                }
            }
            Edit::InsertLine(n) => {
                if self.top_and_bottom_margins.contains(&self.cursor.y)
                    && self.left_and_right_margins.contains(&self.cursor.x)
                {
                    let top_and_bottom_margins = self.cursor.y..self.top_and_bottom_margins.end;
                    let left_and_right_margins = self.left_and_right_margins.clone();
                    self.screen_mut().scroll_down_within_margins(
                        &top_and_bottom_margins,
                        &left_and_right_margins,
                        n as usize,
                    );
                }
            }
            Edit::ScrollDown(n) => self.scroll_down(n as usize),
            Edit::ScrollUp(n) => self.scroll_up(n as usize),
            Edit::EraseInDisplay(erase) => self.erase_in_display(erase),
            Edit::Repeat(n) => {
                let mut y = self.cursor.y;
                let mut x = self.cursor.x;
                let left_and_right_margins = self.left_and_right_margins.clone();
                let top_and_bottom_margins = self.top_and_bottom_margins.clone();

                // Resolve the source cell.  It may be a double-wide character.
                let cell = {
                    let screen = self.screen_mut();
                    let to_copy = x.saturating_sub(1);
                    let line_idx = screen.phys_row(y);
                    let line = screen.line_mut(line_idx);

                    match line.cells().get(to_copy).cloned() {
                        None => Cell::new(' ', CellAttributes::default()),
                        Some(candidate) => {
                            if candidate.str() == " " && to_copy > 0 {
                                // It's a blank.  It may be the second part of
                                // a double-wide pair; look ahead of it.
                                let prior = &line.cells()[to_copy - 1];
                                if prior.width() > 1 {
                                    prior.clone()
                                } else {
                                    candidate
                                }
                            } else {
                                candidate
                            }
                        }
                    }
                };

                for _ in 0..n {
                    {
                        let screen = self.screen_mut();
                        let line_idx = screen.phys_row(y);
                        let line = screen.line_mut(line_idx);

                        line.set_cell(x, cell.clone());
                    }
                    x += 1;
                    if x > left_and_right_margins.end - 1 {
                        x = left_and_right_margins.start;
                        if y == top_and_bottom_margins.end - 1 {
                            self.scroll_up(1);
                        } else {
                            y += 1;
                            if y > top_and_bottom_margins.end - 1 {
                                y = top_and_bottom_margins.end;
                            }
                        }
                    }
                }
                self.cursor.x = x;
                self.cursor.y = y;
            }
        }
    }

    /// https://vt100.net/docs/vt510-rm/DECSLRM.html
    fn set_left_and_right_margins(&mut self, left: OneBased, right: OneBased) {
        // The terminal only recognizes this control function if vertical split
        // screen mode (DECLRMM) is set.
        if self.left_and_right_margin_mode {
            let rows = self.screen().physical_rows as u32;
            let cols = self.screen().physical_cols as u32;
            let left = left.as_zero_based().min(rows - 1).max(0) as usize;
            let right = right.as_zero_based().min(cols - 1).max(0) as usize;

            // The value of the left margin (Pl) must be less than the right margin (Pr).
            if left >= right {
                return;
            }

            // The minimum size of the scrolling region is two columns per DEC,
            // but xterm allows 1.
            /*
            if right - left < 2 {
                return;
            }
            */

            self.left_and_right_margins = left..right + 1;

            // DECSLRM moves the cursor to column 1, line 1 of the page.
            self.set_cursor_pos(&Position::Absolute(0), &Position::Absolute(0));
        }
    }

    fn perform_csi_cursor(&mut self, cursor: Cursor) {
        match cursor {
            Cursor::SetTopAndBottomMargins { top, bottom } => {
                let rows = self.screen().physical_rows;
                let top = i64::from(top.as_zero_based()).min(rows as i64 - 1).max(0);
                let bottom = i64::from(bottom.as_zero_based())
                    .min(rows as i64 - 1)
                    .max(0);
                if top >= bottom {
                    return;
                }
                self.top_and_bottom_margins = top..bottom + 1;
                self.set_cursor_pos(&Position::Absolute(0), &Position::Absolute(0));
                log::debug!(
                    "SetTopAndBottomMargins {:?} (and move cursor to top left: {:?})",
                    self.top_and_bottom_margins,
                    self.cursor
                );
            }

            Cursor::SetLeftAndRightMargins { left, right } => {
                self.set_left_and_right_margins(left, right);
            }

            Cursor::ForwardTabulation(n) => {
                for _ in 0..n {
                    self.c0_horizontal_tab();
                }
            }
            Cursor::BackwardTabulation(n) => {
                for _ in 0..n {
                    let x = match self.tabs.find_prev_tab_stop(self.cursor.x) {
                        Some(x) => x,
                        None => 0,
                    };
                    self.set_cursor_pos(&Position::Absolute(x as i64), &Position::Relative(0));
                }
            }

            Cursor::TabulationClear(to_clear) => {
                self.tabs.clear(to_clear, self.cursor.x);
            }

            Cursor::TabulationControl(_) => {}
            Cursor::LineTabulation(_) => {}

            Cursor::Left(n) => {
                // https://vt100.net/docs/vt510-rm/CUB.html

                let y = self.cursor.y;
                let candidate = self.cursor.x as i64 - n as i64;
                let new_x = if self.cursor.x < self.left_and_right_margins.start {
                    // outside the margin, so allow movement to the border
                    candidate
                } else {
                    // Else constrain to margin
                    if candidate < self.left_and_right_margins.start as i64 {
                        if self.reverse_wraparound_mode && self.dec_auto_wrap {
                            self.left_and_right_margins.end as i64
                                - (self.left_and_right_margins.start as i64 - candidate)
                        } else {
                            self.left_and_right_margins.start as i64
                        }
                    } else {
                        candidate
                    }
                };

                let new_x = new_x.max(0) as usize;

                self.cursor.x = new_x;
                self.wrap_next = false;
                let screen = self.screen_mut();
                screen.dirty_line(y);
            }

            Cursor::Right(n) => {
                // https://vt100.net/docs/vt510-rm/CUF.html
                let y = self.cursor.y;
                let cols = self.screen().physical_cols;
                let new_x = if self.cursor.x >= self.left_and_right_margins.end {
                    // outside the margin, so allow movement to screen edge
                    (self.cursor.x + n as usize).min(cols - 1)
                } else {
                    // Else constrain to margin
                    (self.cursor.x + n as usize).min(self.left_and_right_margins.end - 1)
                };

                self.cursor.x = new_x;
                self.wrap_next = false;
                let screen = self.screen_mut();
                screen.dirty_line(y);
            }

            Cursor::Up(n) => {
                // https://vt100.net/docs/vt510-rm/CUU.html

                let old_y = self.cursor.y;
                let candidate = self.cursor.y.saturating_sub(i64::from(n));
                let new_y = if self.cursor.y < self.top_and_bottom_margins.start {
                    // above the top margin, so allow movement to
                    // top of screen
                    candidate
                } else {
                    // Else constrain to top margin
                    if candidate < self.top_and_bottom_margins.start {
                        self.top_and_bottom_margins.start
                    } else {
                        candidate
                    }
                };

                let new_y = new_y.max(0);

                self.cursor.y = new_y;
                self.wrap_next = false;
                let screen = self.screen_mut();
                screen.dirty_line(old_y);
                screen.dirty_line(new_y);
            }
            Cursor::Down(n) => {
                // https://vt100.net/docs/vt510-rm/CUD.html
                let old_y = self.cursor.y;
                let rows = self.screen().physical_rows;
                let new_y = if self.cursor.y >= self.top_and_bottom_margins.end {
                    // below the bottom margin, so allow movement to
                    // bottom of screen
                    (self.cursor.y + i64::from(n)).min(rows as i64 - 1)
                } else {
                    // Else constrain to bottom margin
                    (self.cursor.y + i64::from(n)).min(self.top_and_bottom_margins.end - 1)
                };

                self.cursor.y = new_y;
                self.wrap_next = false;
                let screen = self.screen_mut();
                screen.dirty_line(old_y);
                screen.dirty_line(new_y);
            }

            Cursor::CharacterAndLinePosition { line, col } | Cursor::Position { line, col } => self
                .set_cursor_pos(
                    &Position::Absolute(i64::from(col.as_zero_based())),
                    &Position::Absolute(i64::from(line.as_zero_based())),
                ),
            Cursor::CharacterAbsolute(col) => self.set_cursor_pos(
                &Position::Absolute(i64::from(col.as_zero_based())),
                &Position::Relative(0),
            ),

            Cursor::CharacterPositionAbsolute(col) => {
                let col = col.as_zero_based() as usize;
                let col = if self.dec_origin_mode {
                    col + self.left_and_right_margins.start
                } else {
                    col
                };
                self.cursor.x = col.min(self.screen().physical_cols - 1);
                self.wrap_next = false;
                let y = self.cursor.y;
                let screen = self.screen_mut();
                screen.dirty_line(y);
            }

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
                // https://vt100.net/docs/vt510-rm/CNL.html
                let old_y = self.cursor.y;
                let rows = self.screen().physical_rows;
                let new_y = if self.cursor.y >= self.top_and_bottom_margins.end {
                    // below the bottom margin, so allow movement to
                    // bottom of screen
                    (self.cursor.y + i64::from(n)).min(rows as i64 - 1)
                } else {
                    // Else constrain to bottom margin
                    (self.cursor.y + i64::from(n)).min(self.top_and_bottom_margins.end - 1)
                };

                self.cursor.y = new_y;
                self.cursor.x = self.left_and_right_margins.start;
                self.wrap_next = false;
                let screen = self.screen_mut();
                screen.dirty_line(old_y);
                screen.dirty_line(new_y);
            }
            Cursor::PrecedingLine(n) => {
                // https://vt100.net/docs/vt510-rm/CPL.html
                let old_y = self.cursor.y;
                let candidate = self.cursor.y.saturating_sub(i64::from(n));
                let new_y = if self.cursor.y < self.top_and_bottom_margins.start {
                    // above the top margin, so allow movement to
                    // top of screen
                    candidate
                } else {
                    // Else constrain to top margin
                    if candidate < self.top_and_bottom_margins.start {
                        self.top_and_bottom_margins.start
                    } else {
                        candidate
                    }
                };

                let new_y = new_y.max(0);

                self.cursor.y = new_y;
                self.cursor.x = self.left_and_right_margins.start;
                self.wrap_next = false;
                let screen = self.screen_mut();
                screen.dirty_line(old_y);
                screen.dirty_line(new_y);
            }

            Cursor::ActivePositionReport { .. } => {
                // This is really a response from the terminal, and
                // we don't need to process it as a terminal command
            }
            Cursor::RequestActivePositionReport => {
                let line = OneBased::from_zero_based(
                    (self.cursor.y.saturating_sub(if self.dec_origin_mode {
                        self.top_and_bottom_margins.start
                    } else {
                        0
                    })) as u32,
                );
                let col = OneBased::from_zero_based(
                    (self.cursor.x.saturating_sub(if self.dec_origin_mode {
                        self.left_and_right_margins.start
                    } else {
                        0
                    })) as u32,
                );
                let report = CSI::Cursor(Cursor::ActivePositionReport { line, col });
                write!(self.writer, "{}", report).ok();
                self.writer.flush().ok();
            }
            Cursor::SaveCursor => {
                // The `CSI s` SaveCursor sequence is ambiguous with DECSLRM
                // with default parameters.  To resolve the ambiguity, DECSLRM
                // is recognized if DECLRMM mode is active which we do here
                // where we have the context!
                if self.left_and_right_margin_mode {
                    // https://vt100.net/docs/vt510-rm/DECSLRM.html
                    self.set_left_and_right_margins(
                        OneBased::new(1),
                        OneBased::new(self.screen().physical_cols as u32 + 1),
                    );
                } else {
                    self.dec_save_cursor();
                }
            }
            Cursor::RestoreCursor => self.dec_restore_cursor(),
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

    /// https://vt100.net/docs/vt510-rm/DECSC.html
    fn dec_save_cursor(&mut self) {
        let saved = SavedCursor {
            position: self.cursor,
            wrap_next: self.wrap_next,
            pen: self.pen.clone(),
            dec_origin_mode: self.dec_origin_mode,
            g0_charset: self.g0_charset,
            g1_charset: self.g1_charset,
        };
        debug!(
            "saving cursor {:?} is_alt={}",
            saved,
            self.screen.is_alt_screen_active()
        );
        *self.screen.saved_cursor() = Some(saved);
    }

    /// https://vt100.net/docs/vt510-rm/DECRC.html
    fn dec_restore_cursor(&mut self) {
        let saved = self
            .screen
            .saved_cursor()
            .clone()
            .unwrap_or_else(|| SavedCursor {
                position: CursorPosition::default(),
                wrap_next: false,
                pen: Default::default(),
                dec_origin_mode: false,
                g0_charset: CharSet::Ascii,
                g1_charset: CharSet::DecLineDrawing,
            });
        debug!(
            "restore cursor {:?} is_alt={}",
            saved,
            self.screen.is_alt_screen_active()
        );
        let x = saved.position.x;
        let y = saved.position.y;
        // Disable origin mode so that we can set the cursor position directly
        self.dec_origin_mode = false;
        self.set_cursor_pos(&Position::Absolute(x as i64), &Position::Absolute(y));
        self.cursor.shape = saved.position.shape;
        self.wrap_next = saved.wrap_next;
        self.pen = saved.pen;
        self.dec_origin_mode = saved.dec_origin_mode;
        self.g0_charset = saved.g0_charset;
        self.g1_charset = saved.g1_charset;
        self.shift_out = false;
    }

    fn perform_csi_sgr(&mut self, sgr: Sgr) {
        debug!("{:?}", sgr);
        match sgr {
            Sgr::Reset => {
                let link = self.pen.hyperlink().map(Arc::clone);
                let semantic_type = self.pen.semantic_type();
                self.pen = CellAttributes::default();
                self.pen.set_hyperlink(link);
                self.pen.set_semantic_type(semantic_type);
            }
            Sgr::Intensity(intensity) => {
                self.pen.set_intensity(intensity);
            }
            Sgr::Underline(underline) => {
                self.pen.set_underline(underline);
            }
            Sgr::Overline(overline) => {
                self.pen.set_overline(overline);
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
            Sgr::UnderlineColor(col) => {
                self.pen.set_underline_color(col);
            }
            Sgr::Font(_) => {}
        }
    }

    /// Computes the set of `SemanticZone`s for the current terminal screen.
    /// Semantic zones are contiguous runs of cells that have the same
    /// `SemanticType` (Prompt, Input, Output).
    /// Due to the way that the terminal clears the screen, the raw, literal
    /// set of zones is overly fragmented by blanks.  This method will ignore
    /// trailing Output regions when computing the SemanticZone bounds.
    ///
    /// By default, all screen data is of type Output.  The shell needs to
    /// employ OSC 133 escapes to markup its output.
    pub fn get_semantic_zones(&self) -> anyhow::Result<Vec<SemanticZone>> {
        let screen = self.screen();

        let mut last_cell: Option<&Cell> = None;
        let mut current_zone = None;
        let mut zones = vec![];
        let blank_cell = Cell::new(' ', Default::default());

        for (idx, line) in screen.lines.iter().enumerate() {
            let stable_row = screen.phys_to_stable_row_index(idx);

            // Rows may have trailing space+Output cells interleaved
            // with other zones as a result of clear-to-eol and
            // clear-to-end-of-screen sequences.  We don't want
            // those to affect the zones that we compute here
            let last_non_blank = line
                .cells()
                .iter()
                .rposition(|cell| *cell != blank_cell)
                .unwrap_or(line.cells().len());

            for (grapheme_idx, cell) in line.visible_cells() {
                if grapheme_idx > last_non_blank {
                    break;
                }
                let semantic_type = cell.attrs().semantic_type();
                let new_zone = match last_cell {
                    None => true,
                    Some(c) => c.attrs().semantic_type() != semantic_type,
                };

                if new_zone {
                    if let Some(zone) = current_zone.take() {
                        zones.push(zone);
                    }

                    current_zone.replace(SemanticZone {
                        start_x: grapheme_idx as _,
                        start_y: stable_row,
                        end_x: grapheme_idx as _,
                        end_y: stable_row,
                        semantic_type: semantic_type,
                    });
                }

                if let Some(zone) = current_zone.as_mut() {
                    zone.end_x = grapheme_idx as _;
                    zone.end_y = stable_row;
                }

                last_cell.replace(cell);
            }
        }
        if let Some(zone) = current_zone.take() {
            zones.push(zone);
        }

        Ok(zones)
    }
}

/// A helper struct for implementing `vtparse::VTActor` while compartmentalizing
/// the terminal state and the embedding/host terminal interface
pub(crate) struct Performer<'a> {
    pub state: &'a mut TerminalState,
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
        self.flush_print(false);
    }
}

fn selection_to_selection(sel: Selection) -> ClipboardSelection {
    match sel {
        Selection::CLIPBOARD => ClipboardSelection::Clipboard,
        Selection::PRIMARY => ClipboardSelection::PrimarySelection,
        // xterm will use a configurable selection in the NONE case
        Selection::NONE => ClipboardSelection::Clipboard,
        // otherwise we just use clipboard.  Could potentially
        // also use the same fallback configuration as NONE,
        // if/when we add it
        _ => ClipboardSelection::Clipboard,
    }
}

impl<'a> Performer<'a> {
    pub fn new(state: &'a mut TerminalState) -> Self {
        Self { state, print: None }
    }

    fn flush_print(&mut self, cr_follows: bool) {
        let p = match self.print.take() {
            Some(s) => s,
            None => return,
        };

        let mut graphemes =
            unicode_segmentation::UnicodeSegmentation::graphemes(p.as_str(), true).peekable();

        while let Some(g) = graphemes.next() {
            let g = if (self.shift_out && self.g1_charset == CharSet::DecLineDrawing)
                || (!self.shift_out && self.g0_charset == CharSet::DecLineDrawing)
            {
                match g {
                    "`" => "◆",
                    "a" => "▒",
                    "b" => "␉",
                    "c" => "␌",
                    "d" => "␍",
                    "e" => "␊",
                    "f" => "°",
                    "g" => "±",
                    "h" => "␤",
                    "i" => "␋",
                    "j" => "┘",
                    "k" => "┐",
                    "l" => "┌",
                    "m" => "└",
                    "n" => "┼",
                    "o" => "⎺",
                    "p" => "⎻",
                    "q" => "─",
                    "r" => "⎼",
                    "s" => "⎽",
                    "t" => "├",
                    "u" => "┤",
                    "v" => "┴",
                    "w" => "┬",
                    "x" => "│",
                    "y" => "≤",
                    "z" => "≥",
                    "{" => "π",
                    "|" => "≠",
                    "}" => "£",
                    "~" => "·",
                    _ => g,
                }
            } else if (self.shift_out && self.g1_charset == CharSet::Uk)
                || (!self.shift_out && self.g0_charset == CharSet::Uk)
            {
                match g {
                    "#" => "£",
                    _ => g,
                }
            } else {
                g
            };

            if self.wrap_next {
                self.new_line(true);
            }

            let x = self.cursor.x;
            let y = self.cursor.y;
            let width = self.left_and_right_margins.end;

            let mut pen = self.pen.clone();
            // the max(1) here is to ensure that we advance to the next cell
            // position for zero-width graphemes.  We want to make sure that
            // they occupy a cell so that we can re-emit them when we output them.
            // If we didn't do this, then we'd effectively filter them out from
            // the model, which seems like a lossy design choice.
            let print_width = unicode_column_width(g).max(1);
            let is_last = graphemes.peek().is_none();

            // We're going to mark the cell as being wrapped, but not if this grapheme
            // is the last in this run and we know that we're followed by a CR.
            // In that case, we know that there is an explicit line break and
            // we mustn't record a wrap for that!
            let wrappable = (x + print_width >= width) && !(is_last && cr_follows);

            if wrappable {
                pen.set_wrapped(true);
            }

            let cell = Cell::new_grapheme(g, pen);

            if self.insert {
                let margin = self.left_and_right_margins.end;
                let screen = self.screen_mut();
                for _ in x..x + print_width as usize {
                    screen.insert_cell(x, y, margin);
                }
            }

            // Assign the cell
            log::trace!(
                "print x={} y={} is_last={} cr_follows={} print_width={} width={} cell={:?}",
                x,
                y,
                is_last,
                cr_follows,
                print_width,
                width,
                cell
            );
            self.screen_mut().set_cell(x, y, &cell);

            if !wrappable {
                self.cursor.x += print_width;
                self.wrap_next = false;
            } else {
                self.wrap_next = self.dec_auto_wrap;
            }
        }
    }

    pub fn perform(&mut self, action: Action) {
        debug!("perform {:?}", action);
        match action {
            Action::Print(c) => self.print(c),
            Action::Control(code) => self.control(code),
            Action::DeviceControl(ctrl) => self.device_control(ctrl),
            Action::OperatingSystemCommand(osc) => self.osc_dispatch(*osc),
            Action::Esc(esc) => self.esc_dispatch(esc),
            Action::CSI(csi) => self.csi_dispatch(csi),
            Action::Sixel(sixel) => self.sixel(sixel),
        }
    }

    fn device_control(&mut self, ctrl: DeviceControlMode) {
        match &ctrl {
            DeviceControlMode::ShortDeviceControl(s) => {
                match (s.byte, s.intermediates.as_slice()) {
                    (b'q', &[b'$']) => {
                        // DECRQSS - Request Status String
                        // https://vt100.net/docs/vt510-rm/DECRQSS.html
                        // The response is described here:
                        // https://vt100.net/docs/vt510-rm/DECRPSS.html
                        // but note that *that* text has the validity value
                        // inverted; there's a note about this in the xterm
                        // ctlseqs docs.
                        match s.data.as_slice() {
                            &[b'"', b'p'] => {
                                // DECSCL - select conformance level
                                write!(self.writer, "{}1$r65;1\"p{}", DCS, ST).ok();
                                self.writer.flush().ok();
                            }
                            &[b'r'] => {
                                // DECSTBM - top and bottom margins
                                let margins = self.top_and_bottom_margins.clone();
                                write!(
                                    self.writer,
                                    "{}1$r{};{}r{}",
                                    DCS,
                                    margins.start + 1,
                                    margins.end,
                                    ST
                                )
                                .ok();
                                self.writer.flush().ok();
                            }
                            &[b's'] => {
                                // DECSLRM - left and right margins
                                let margins = self.left_and_right_margins.clone();
                                write!(
                                    self.writer,
                                    "{}1$r{};{}s{}",
                                    DCS,
                                    margins.start + 1,
                                    margins.end,
                                    ST
                                )
                                .ok();
                                self.writer.flush().ok();
                            }
                            _ => {
                                log::warn!("unhandled DECRQSS {:?}", s);
                                // Reply that the request is invalid
                                write!(self.writer, "{}0$r{}", DCS, ST).ok();
                                self.writer.flush().ok();
                            }
                        }
                    }
                    _ => log::warn!("unhandled {:?}", s),
                }
            }
            _ => match self.device_control_handler.as_mut() {
                Some(handler) => handler.handle_device_control(ctrl),
                None => log::warn!("unhandled {:?}", ctrl),
            },
        }
    }

    /// Draw a character to the screen
    fn print(&mut self, c: char) {
        // We buffer up the chars to increase the chances of correctly grouping graphemes into cells
        self.print.get_or_insert_with(String::new).push(c);
    }

    fn control(&mut self, control: ControlCode) {
        let cr_follows = matches!(control, ControlCode::CarriageReturn);
        self.flush_print(cr_follows);
        match control {
            ControlCode::LineFeed | ControlCode::VerticalTab | ControlCode::FormFeed => {
                if self.left_and_right_margins.contains(&self.cursor.x) {
                    self.new_line(false);
                } else {
                    // Do move down, but don't trigger a scroll when we're
                    // outside of the left/right margins
                    let old_y = self.cursor.y;
                    let y = if old_y == self.top_and_bottom_margins.end - 1 {
                        old_y
                    } else {
                        (old_y + 1).min(self.screen().physical_rows as i64 - 1)
                    };
                    self.screen_mut().dirty_line(old_y);
                    self.screen_mut().dirty_line(y);
                    self.cursor.y = y;
                    self.wrap_next = false;
                }
            }
            ControlCode::CarriageReturn => {
                if self.cursor.x >= self.left_and_right_margins.start {
                    self.cursor.x = self.left_and_right_margins.start;
                } else {
                    self.cursor.x = 0;
                }
                let y = self.cursor.y;
                self.wrap_next = false;
                self.screen_mut().dirty_line(y);
            }

            ControlCode::Backspace => {
                if self.reverse_wraparound_mode
                    && self.dec_auto_wrap
                    && self.cursor.x == self.left_and_right_margins.start
                    && self.cursor.y == self.top_and_bottom_margins.start
                {
                    // Backspace off the top-left wraps around to the bottom right
                    let x_pos = Position::Absolute(self.left_and_right_margins.end as i64 - 1);
                    let y_pos = Position::Absolute(self.top_and_bottom_margins.end - 1);
                    self.set_cursor_pos(&x_pos, &y_pos);
                } else if self.reverse_wraparound_mode
                    && self.dec_auto_wrap
                    && self.cursor.x <= self.left_and_right_margins.start
                {
                    // Backspace off the left wraps around to the prior line on the right
                    let x_pos = Position::Absolute(self.left_and_right_margins.end as i64 - 1);
                    let y_pos = Position::Relative(-1);
                    self.set_cursor_pos(&x_pos, &y_pos);
                } else if self.reverse_wraparound_mode
                    && self.dec_auto_wrap
                    && self.cursor.x == self.left_and_right_margins.end - 1
                    && self.wrap_next
                {
                    // If the cursor is in the last column and a character was
                    // just output and reverse-wraparound is on then backspace
                    // by 1 has no effect.
                } else if self.cursor.x == self.left_and_right_margins.start {
                    // Respect the left margin and don't BS outside it
                } else {
                    self.set_cursor_pos(&Position::Relative(-1), &Position::Relative(0));
                }
            }
            ControlCode::HorizontalTab => self.c0_horizontal_tab(),
            ControlCode::HTS => self.c1_hts(),
            ControlCode::IND => self.c1_index(),
            ControlCode::NEL => self.c1_nel(),
            ControlCode::Bell => {
                if let Some(handler) = self.alert_handler.as_mut() {
                    handler.alert(Alert::Bell);
                } else {
                    log::info!("Ding! (this is the bell)");
                }
            }
            ControlCode::RI => self.c1_reverse_index(),

            // wezterm only supports UTF-8, so does not support the
            // DEC National Replacement Character Sets.  However, it does
            // support the DEC Special Graphics character set used by
            // numerous ncurses applications.  DEC Special Graphics can be
            // selected by ASCII Shift Out (0x0E, ^N) or by setting G0
            // via ESC ( 0 .
            ControlCode::ShiftIn => {
                self.shift_out = false;
            }
            ControlCode::ShiftOut => {
                self.shift_out = true;
            }
            _ => log::warn!("unhandled ControlCode {:?}", control),
        }
    }

    fn csi_dispatch(&mut self, csi: CSI) {
        self.flush_print(false);
        match csi {
            CSI::Sgr(sgr) => self.state.perform_csi_sgr(sgr),
            CSI::Cursor(cursor) => self.state.perform_csi_cursor(cursor),
            CSI::Edit(edit) => self.state.perform_csi_edit(edit),
            CSI::Mode(mode) => self.state.perform_csi_mode(mode),
            CSI::Device(dev) => self.state.perform_device(*dev),
            CSI::Mouse(mouse) => error!("mouse report sent by app? {:?}", mouse),
            CSI::Window(window) => self.state.perform_csi_window(window),
            CSI::Unspecified(unspec) => {
                log::warn!("unknown unspecified CSI: {:?}", format!("{}", unspec))
            }
        };
    }

    fn esc_dispatch(&mut self, esc: Esc) {
        self.flush_print(false);
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
            Esc::Code(EscCode::DecLineDrawingG0) => {
                self.g0_charset = CharSet::DecLineDrawing;
            }
            Esc::Code(EscCode::AsciiCharacterSetG0) => {
                self.g0_charset = CharSet::Ascii;
            }
            Esc::Code(EscCode::UkCharacterSetG0) => {
                self.g0_charset = CharSet::Uk;
            }
            Esc::Code(EscCode::DecLineDrawingG1) => {
                self.g1_charset = CharSet::DecLineDrawing;
            }
            Esc::Code(EscCode::AsciiCharacterSetG1) => {
                self.g1_charset = CharSet::Ascii;
            }
            Esc::Code(EscCode::UkCharacterSetG1) => {
                self.g1_charset = CharSet::Uk;
            }
            Esc::Code(EscCode::DecSaveCursorPosition) => self.dec_save_cursor(),
            Esc::Code(EscCode::DecRestoreCursorPosition) => self.dec_restore_cursor(),

            Esc::Code(EscCode::DecScreenAlignmentDisplay) => {
                // This one is just to make vttest happy;
                // its original purpose was for aligning the CRT.
                // https://vt100.net/docs/vt510-rm/DECALN.html

                let screen = self.screen_mut();
                let col_range = 0..screen.physical_cols;
                for y in 0..screen.physical_rows as VisibleRowIndex {
                    let line_idx = screen.phys_row(y);
                    let line = screen.line_mut(line_idx);
                    line.resize(col_range.end);
                    line.fill_range(
                        col_range.clone(),
                        &Cell::new('E', CellAttributes::default()),
                    );
                }

                self.top_and_bottom_margins = 0..self.screen().physical_rows as VisibleRowIndex;
                self.left_and_right_margins = 0..self.screen().physical_cols;
                self.cursor = Default::default();
            }

            // RIS resets a device to its initial state, i.e. the state it has after it is switched
            // on. This may imply, if applicable: remove tabulation stops, remove qualified areas,
            // reset graphic rendition, erase all positions, move active position to first
            // character position of first line.
            Esc::Code(EscCode::FullReset) => {
                self.pen = Default::default();
                self.cursor = Default::default();
                self.wrap_next = false;
                self.insert = false;
                self.dec_auto_wrap = true;
                self.reverse_wraparound_mode = false;
                self.dec_origin_mode = false;
                self.use_private_color_registers_for_each_graphic = false;
                self.color_map = default_color_map();
                self.application_cursor_keys = false;
                self.sixel_scrolling = true;
                self.dec_ansi_mode = false;
                self.application_keypad = false;
                self.bracketed_paste = false;
                self.focus_tracking = false;
                self.sgr_mouse = false;
                self.sixel_scrolls_right = false;
                self.any_event_mouse = false;
                self.button_event_mouse = false;
                self.current_mouse_button = MouseButton::None;
                self.cursor_visible = true;
                self.g0_charset = CharSet::Ascii;
                self.g1_charset = CharSet::DecLineDrawing;
                self.shift_out = false;
                self.tabs = TabStop::new(self.screen().physical_cols, 8);
                self.palette.take();
                self.top_and_bottom_margins = 0..self.screen().physical_rows as VisibleRowIndex;
                self.left_and_right_margins = 0..self.screen().physical_cols;

                self.screen.activate_primary_screen();
                self.erase_in_display(EraseInDisplay::EraseScrollback);
                self.erase_in_display(EraseInDisplay::EraseDisplay);
                if let Some(handler) = self.alert_handler.as_mut() {
                    handler.alert(Alert::PaletteChanged);
                }
            }

            _ => log::warn!("ESC: unhandled {:?}", esc),
        }
    }

    fn osc_dispatch(&mut self, osc: OperatingSystemCommand) {
        self.flush_print(false);
        match osc {
            OperatingSystemCommand::SetIconNameSun(title)
            | OperatingSystemCommand::SetIconName(title) => {
                if title.is_empty() {
                    self.icon_title = None;
                } else {
                    self.icon_title = Some(title.clone());
                }
                if let Some(handler) = self.alert_handler.as_mut() {
                    handler.alert(Alert::TitleMaybeChanged);
                }
            }
            OperatingSystemCommand::SetIconNameAndWindowTitle(title) => {
                self.icon_title.take();
                self.title = title.clone();
                if let Some(handler) = self.alert_handler.as_mut() {
                    handler.alert(Alert::TitleMaybeChanged);
                }
            }

            OperatingSystemCommand::SetWindowTitleSun(title)
            | OperatingSystemCommand::SetWindowTitle(title) => {
                self.title = title.clone();
                if let Some(handler) = self.alert_handler.as_mut() {
                    handler.alert(Alert::TitleMaybeChanged);
                }
            }
            OperatingSystemCommand::SetHyperlink(link) => {
                self.set_hyperlink(link);
            }
            OperatingSystemCommand::Unspecified(unspec) => {
                let mut output = String::new();
                write!(&mut output, "Unhandled OSC ").ok();
                for item in unspec {
                    write!(&mut output, " {}", String::from_utf8_lossy(&item)).ok();
                }
                log::warn!("{}", output);
            }

            OperatingSystemCommand::ClearSelection(selection) => {
                let selection = selection_to_selection(selection);
                self.set_clipboard_contents(selection, None).ok();
            }
            OperatingSystemCommand::QuerySelection(_) => {}
            OperatingSystemCommand::SetSelection(selection, selection_data) => {
                let selection = selection_to_selection(selection);
                match self.set_clipboard_contents(selection, Some(selection_data)) {
                    Ok(_) => (),
                    Err(err) => error!("failed to set clipboard in response to OSC 52: {:?}", err),
                }
            }
            OperatingSystemCommand::ITermProprietary(iterm) => match iterm {
                ITermProprietary::File(image) => self.set_image(*image),
                ITermProprietary::SetUserVar { name, value } => {
                    self.user_vars.insert(name, value);
                    if let Some(handler) = self.alert_handler.as_mut() {
                        handler.alert(Alert::TitleMaybeChanged);
                    }
                }
                _ => log::warn!("unhandled iterm2: {:?}", iterm),
            },

            OperatingSystemCommand::FinalTermSemanticPrompt(FinalTermSemanticPrompt::FreshLine) => {
                self.fresh_line();
            }
            OperatingSystemCommand::FinalTermSemanticPrompt(
                FinalTermSemanticPrompt::FreshLineAndStartPrompt { .. },
            ) => {
                self.fresh_line();
                self.pen.set_semantic_type(SemanticType::Prompt);
            }
            OperatingSystemCommand::FinalTermSemanticPrompt(
                FinalTermSemanticPrompt::StartPrompt(_),
            ) => {
                self.pen.set_semantic_type(SemanticType::Prompt);
            }
            OperatingSystemCommand::FinalTermSemanticPrompt(
                FinalTermSemanticPrompt::MarkEndOfCommandWithFreshLine { .. },
            ) => {
                self.fresh_line();
                self.pen.set_semantic_type(SemanticType::Prompt);
            }
            OperatingSystemCommand::FinalTermSemanticPrompt(
                FinalTermSemanticPrompt::MarkEndOfPromptAndStartOfInputUntilNextMarker { .. },
            ) => {
                self.pen.set_semantic_type(SemanticType::Input);
            }
            OperatingSystemCommand::FinalTermSemanticPrompt(
                FinalTermSemanticPrompt::MarkEndOfInputAndStartOfOutput { .. },
            ) => {
                self.pen.set_semantic_type(SemanticType::Output);
            }

            OperatingSystemCommand::FinalTermSemanticPrompt(
                FinalTermSemanticPrompt::CommandStatus { .. },
            ) => {}

            OperatingSystemCommand::FinalTermSemanticPrompt(ft) => {
                log::warn!("unhandled: {:?}", ft);
            }

            OperatingSystemCommand::SystemNotification(message) => {
                if let Some(handler) = self.alert_handler.as_mut() {
                    handler.alert(Alert::ToastNotification {
                        title: None,
                        body: message,
                        focus: true,
                    });
                } else {
                    log::info!("Application sends SystemNotification: {}", message);
                }
            }
            OperatingSystemCommand::RxvtExtension(params) => {
                if let Some("notify") = params.get(0).map(String::as_str) {
                    let title = params.get(1);
                    let body = params.get(2);
                    let (title, body) = match (title.cloned(), body.cloned()) {
                        (Some(title), None) => (None, title),
                        (Some(title), Some(body)) => (Some(title), body),
                        _ => {
                            log::warn!("malformed rxvt notify escape: {:?}", params);
                            return;
                        }
                    };
                    if let Some(handler) = self.alert_handler.as_mut() {
                        handler.alert(Alert::ToastNotification {
                            title,
                            body,
                            focus: true,
                        });
                    }
                }
            }
            OperatingSystemCommand::CurrentWorkingDirectory(url) => {
                self.current_dir = Url::parse(&url).ok();
                if let Some(handler) = self.alert_handler.as_mut() {
                    handler.alert(Alert::TitleMaybeChanged);
                }
            }
            OperatingSystemCommand::ChangeColorNumber(specs) => {
                log::trace!("ChangeColorNumber: {:?}", specs);
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
                            write!(self.writer, "{}", response).ok();
                            self.writer.flush().ok();
                        }
                        ColorOrQuery::Color(c) => {
                            self.palette_mut().colors.0[pair.palette_index as usize] = c;
                        }
                    }
                }
                if let Some(handler) = self.alert_handler.as_mut() {
                    handler.alert(Alert::PaletteChanged);
                }
                self.make_all_lines_dirty();
            }

            OperatingSystemCommand::ResetColors(colors) => {
                log::trace!("ResetColors: {:?}", colors);
                if colors.is_empty() {
                    // Reset all colors
                    self.palette.take();
                } else {
                    // Reset individual colors
                    if self.palette.is_none() {
                        // Already at the defaults
                    } else {
                        let base = self.config.color_palette();
                        for c in colors {
                            let c = c as usize;
                            self.palette_mut().colors.0[c] = base.colors.0[c];
                        }
                    }
                }
                if let Some(handler) = self.alert_handler.as_mut() {
                    handler.alert(Alert::PaletteChanged);
                }
            }

            OperatingSystemCommand::ChangeDynamicColors(first_color, colors) => {
                log::trace!("ChangeDynamicColors: {:?} {:?}", first_color, colors);
                use termwiz::escape::osc::DynamicColorNumber;
                let mut idx: u8 = first_color as u8;
                for color in colors {
                    let which_color: Option<DynamicColorNumber> = FromPrimitive::from_u8(idx);
                    log::trace!("ChangeDynamicColors item: {:?}", which_color);
                    if let Some(which_color) = which_color {
                        macro_rules! set_or_query {
                            ($name:ident) => {
                                match color {
                                    ColorOrQuery::Query => {
                                        let response = OperatingSystemCommand::ChangeDynamicColors(
                                            which_color,
                                            vec![ColorOrQuery::Color(self.palette().$name)],
                                        );
                                        log::trace!("Color Query response {:?}", response);
                                        write!(self.writer, "{}", response).ok();
                                        self.writer.flush().ok();
                                    }
                                    ColorOrQuery::Color(c) => self.palette_mut().$name = c,
                                }
                            };
                        }
                        match which_color {
                            DynamicColorNumber::TextForegroundColor => set_or_query!(foreground),
                            DynamicColorNumber::TextBackgroundColor => set_or_query!(background),
                            DynamicColorNumber::TextCursorColor => {
                                if let ColorOrQuery::Color(c) = color {
                                    // We set the border to the background color; we don't
                                    // have an escape that sets that independently, and this
                                    // way just looks better.
                                    self.palette_mut().cursor_border = c;
                                }
                                set_or_query!(cursor_bg)
                            }
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
                if let Some(handler) = self.alert_handler.as_mut() {
                    handler.alert(Alert::PaletteChanged);
                }
                self.make_all_lines_dirty();
            }

            OperatingSystemCommand::ResetDynamicColor(color) => {
                log::trace!("ResetDynamicColor: {:?}", color);
                use termwiz::escape::osc::DynamicColorNumber;
                let which_color: Option<DynamicColorNumber> = FromPrimitive::from_u8(color as u8);
                if let Some(which_color) = which_color {
                    macro_rules! reset {
                        ($name:ident) => {
                            if self.palette.is_none() {
                                // Already at the defaults
                            } else {
                                let base = self.config.color_palette();
                                self.palette_mut().$name = base.$name;
                            }
                        };
                    }
                    match which_color {
                        DynamicColorNumber::TextForegroundColor => reset!(foreground),
                        DynamicColorNumber::TextBackgroundColor => reset!(background),
                        DynamicColorNumber::TextCursorColor => {
                            reset!(cursor_bg);
                            // Since we set the border to the bg, we consider it reset
                            // by resetting the bg too!
                            reset!(cursor_border);
                        }
                        DynamicColorNumber::HighlightForegroundColor => reset!(selection_fg),
                        DynamicColorNumber::HighlightBackgroundColor => reset!(selection_bg),
                        DynamicColorNumber::MouseForegroundColor
                        | DynamicColorNumber::MouseBackgroundColor
                        | DynamicColorNumber::TektronixForegroundColor
                        | DynamicColorNumber::TektronixBackgroundColor
                        | DynamicColorNumber::TektronixCursorColor => {}
                    }
                }
                if let Some(handler) = self.alert_handler.as_mut() {
                    handler.alert(Alert::PaletteChanged);
                }
                self.make_all_lines_dirty();
            }
        }
    }
}
