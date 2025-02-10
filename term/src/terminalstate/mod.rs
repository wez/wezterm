// The range_plus_one lint can't see when the LHS is not compatible with
// and inclusive range
#![allow(clippy::range_plus_one)]
use super::*;
use crate::color::{ColorPalette, RgbColor};
use crate::config::{BidiMode, NewlineCanon};
use log::debug;
use num_traits::ToPrimitive;
use std::collections::HashMap;
use std::io::{BufWriter, Write};
use std::num::NonZeroUsize;
use std::sync::mpsc::{channel, Sender};
use std::sync::Arc;
use terminfo::{Database, Value};
use termwiz::cell::UnicodeVersion;
use termwiz::escape::csi::{
    Cursor, CursorStyle, DecPrivateMode, DecPrivateModeCode, Device, Edit, EraseInDisplay,
    EraseInLine, Mode, Sgr, TabulationClear, TerminalMode, TerminalModeCode, Window, XtSmGraphics,
    XtSmGraphicsAction, XtSmGraphicsItem, XtSmGraphicsStatus, XtermKeyModifierResource,
};
use termwiz::escape::{OneBased, OperatingSystemCommand, CSI};
use termwiz::image::ImageData;
use termwiz::input::KeyboardEncoding;
use termwiz::surface::{CursorShape, CursorVisibility, SequenceNo};
use url::Url;
use wezterm_bidi::ParagraphDirectionHint;

mod image;
mod iterm;
mod keyboard;
mod kitty;
mod mouse;
pub(crate) mod performer;
mod sixel;
use crate::terminalstate::image::*;
use crate::terminalstate::kitty::*;

lazy_static::lazy_static! {
    static ref DB: Database = {
        let data = include_bytes!("../../../termwiz/data/wezterm");
        Database::from_buffer(&data[..]).unwrap()
    };
}

pub(crate) struct TabStop {
    tabs: Vec<bool>,
    tab_width: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CharSet {
    Ascii,
    Uk,
    DecLineDrawing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MouseEncoding {
    X10,
    Utf8,
    SGR,
    SgrPixels,
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

    fn clear(&mut self, to_clear: TabulationClear, col: usize, log_unknown_escape_sequences: bool) {
        match to_clear {
            TabulationClear::ClearCharacterTabStopAtActivePosition => {
                if let Some(t) = self.tabs.get_mut(col) {
                    *t = false;
                }
            }
            // If we want to exactly match VT100/xterm behavior, then
            // we cannot honor ClearCharacterTabStopsAtActiveLine.
            TabulationClear::ClearAllCharacterTabStops => {
                // | TabulationClear::ClearCharacterTabStopsAtActiveLine
                for t in &mut self.tabs {
                    *t = false;
                }
            }
            _ => {
                if log_unknown_escape_sequences {
                    log::warn!("unhandled TabulationClear {:?}", to_clear);
                }
            }
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
        size: TerminalSize,
        config: &Arc<dyn TerminalConfiguration>,
        seqno: SequenceNo,
        bidi_mode: BidiMode,
    ) -> Self {
        let screen = Screen::new(size, config, true, seqno, bidi_mode);
        let alt_screen = Screen::new(size, config, false, seqno, bidi_mode);

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
        size: TerminalSize,
        cursor_main: CursorPosition,
        cursor_alt: CursorPosition,
        seqno: SequenceNo,
        is_conpty: bool,
    ) -> (CursorPosition, CursorPosition) {
        let cursor_main = self.screen.resize(size, cursor_main, seqno, is_conpty);
        let cursor_alt = self.alt_screen.resize(size, cursor_alt, seqno, is_conpty);
        (cursor_main, cursor_alt)
    }

    pub fn activate_alt_screen(&mut self, seqno: SequenceNo) {
        self.alt_screen_is_active = true;
        self.dirty_top_phys_rows(seqno);
    }

    pub fn activate_primary_screen(&mut self, seqno: SequenceNo) {
        self.alt_screen_is_active = false;
        self.dirty_top_phys_rows(seqno);
    }

    // When switching between alt and primary screen, we implicitly change
    // the content associated with StableRowIndex 0..num_rows.  The muxer
    // use case needs to know to invalidate its cache, so we mark those rows
    // as dirty.
    fn dirty_top_phys_rows(&mut self, seqno: SequenceNo) {
        let num_rows = self.screen.physical_rows;
        for line_idx in 0..num_rows {
            self.screen
                .line_mut(line_idx)
                .update_last_change_seqno(seqno);
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

    pub fn full_reset(&mut self) {
        self.screen.full_reset();
        self.alt_screen.full_reset();
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

    clear_semantic_attribute_on_newline: bool,

    /// If true, writing a character inserts a new cell
    insert: bool,

    /// https://vt100.net/docs/vt510-rm/DECAWM.html
    dec_auto_wrap: bool,

    /// Reverse Wraparound Mode
    reverse_wraparound_mode: bool,

    /// Reverse video mode
    reverse_video_mode: bool,

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
    modify_other_keys: Option<i64>,

    dec_ansi_mode: bool,

    /// https://vt100.net/dec/ek-vt38t-ug-001.pdf#page=132 has a
    /// discussion on what sixel dispay mode (DECSDM) does.
    sixel_display_mode: bool,
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
    /// X10 (legacy), SGR, and SGR-Pixels style mouse tracking and
    /// reporting is enabled
    mouse_encoding: MouseEncoding,
    mouse_tracking: bool,
    /// Button events enabled
    button_event_mouse: bool,
    current_mouse_buttons: Vec<MouseButton>,
    last_mouse_move: Option<MouseEvent>,
    cursor_visible: bool,

    keyboard_encoding: KeyboardEncoding,
    /// Support for US, UK, and DEC Special Graphics
    g0_charset: CharSet,
    g1_charset: CharSet,
    shift_out: bool,

    newline_mode: bool,

    tabs: TabStop,

    /// The terminal title string (OSC 2)
    title: String,
    /// The icon title string (OSC 1)
    icon_title: Option<String>,
    progress: Progress,

    palette: Option<ColorPalette>,

    pixel_width: usize,
    pixel_height: usize,
    dpi: u32,

    clipboard: Option<Arc<dyn Clipboard>>,
    device_control_handler: Option<Box<dyn DeviceControlHandler>>,
    alert_handler: Option<Box<dyn AlertHandler>>,
    download_handler: Option<Arc<dyn DownloadHandler>>,

    current_dir: Option<Url>,

    term_program: String,
    term_version: String,

    writer: BufWriter<ThreadedWriter>,

    image_cache: lru::LruCache<[u8; 32], Arc<ImageData>>,
    sixel_scrolls_right: bool,

    user_vars: HashMap<String, String>,

    kitty_img: KittyImageState,
    seqno: SequenceNo,

    /// The unicode version that is in effect
    unicode_version: UnicodeVersion,
    unicode_version_stack: Vec<UnicodeVersionStackEntry>,

    enable_conpty_quirks: bool,
    /// On Windows, the ConPTY layer emits an OSC sequence to
    /// set the title shortly after it starts up.
    /// We don't want that, so we use this flag to remember
    /// whether we want to skip it or not.
    suppress_initial_title_change: bool,

    accumulating_title: Option<String>,

    /// seqno when we last lost focus
    lost_focus_seqno: SequenceNo,
    /// seqno when we last emitted Alert::OutputSinceFocusLost
    lost_focus_alerted_seqno: SequenceNo,
    focused: bool,

    /// True if lines should be marked as bidi-enabled, and thus
    /// have the renderer apply the bidi algorithm.
    /// true is equivalent to "implicit" bidi mode as described in
    /// <https://terminal-wg.pages.freedesktop.org/bidi/recommendation/basic-modes.html>
    /// If none, then the default value specified by the config is used.
    bidi_enabled: Option<bool>,
    /// When set, specifies the bidi direction information that should be
    /// applied to lines.
    /// If none, then the default value specified by the config is used.
    bidi_hint: Option<ParagraphDirectionHint>,
}

#[derive(Debug)]
struct UnicodeVersionStackEntry {
    vers: UnicodeVersion,
    label: Option<String>,
}

fn default_color_map() -> HashMap<u16, RgbColor> {
    let mut color_map = HashMap::new();
    // Match colors to the VT340 color table:
    // https://github.com/hackerb9/vt340test/blob/main/colormap/showcolortable.png
    for (idx, r, g, b) in [
        (0, 0, 0, 0),
        (1, 0x33, 0x33, 0xcc),
        (2, 0xcc, 0x23, 0x23),
        (3, 0x33, 0xcc, 0x33),
        (4, 0xcc, 0x33, 0xcc),
        (5, 0x33, 0xcc, 0xcc),
        (6, 0xcc, 0xcc, 0xcc),
        (7, 0x77, 0x77, 0x77),
        (8, 0x44, 0x44, 0x44),
        (9, 0x56, 0x56, 0x99),
        (10, 0x99, 0x44, 0x44),
        (11, 0x56, 0x99, 0x56),
        (12, 0x99, 0x56, 0x99),
        (13, 0x56, 0x99, 0x99),
        (14, 0x99, 0x99, 0x56),
        (15, 0xcc, 0xcc, 0xcc),
    ] {
        color_map.insert(idx, RgbColor::new_8bpc(r, g, b));
    }
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
    sender: Sender<WriterMessage>,
}

enum WriterMessage {
    Data(Vec<u8>),
    Flush,
}

impl ThreadedWriter {
    fn new(mut writer: Box<dyn std::io::Write + Send>) -> Self {
        let (sender, receiver) = channel::<WriterMessage>();

        std::thread::spawn(move || {
            while let Ok(msg) = receiver.recv() {
                match msg {
                    WriterMessage::Data(buf) => {
                        if writer.write(&buf).is_err() {
                            break;
                        }
                    }
                    WriterMessage::Flush => {
                        if writer.flush().is_err() {
                            break;
                        }
                    }
                }
            }
        });

        Self { sender }
    }
}

impl std::io::Write for ThreadedWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.sender
            .send(WriterMessage::Data(buf.to_vec()))
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::BrokenPipe, err))?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.sender
            .send(WriterMessage::Flush)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::BrokenPipe, err))?;
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
        let writer = BufWriter::new(ThreadedWriter::new(writer));
        let seqno = 1;
        let screen = ScreenOrAlt::new(size, &config, seqno, config.bidi_mode());

        let color_map = default_color_map();

        let unicode_version = config.unicode_version();

        TerminalState {
            config,
            screen,
            pen: CellAttributes::default(),
            cursor: CursorPosition::default(),
            top_and_bottom_margins: 0..size.rows as VisibleRowIndex,
            left_and_right_margins: 0..size.cols,
            left_and_right_margin_mode: false,
            wrap_next: false,
            clear_semantic_attribute_on_newline: false,
            // We default auto wrap to true even though the default for
            // a dec terminal is false, because it is more useful this way.
            dec_auto_wrap: true,
            reverse_wraparound_mode: false,
            reverse_video_mode: false,
            dec_origin_mode: false,
            insert: false,
            application_cursor_keys: false,
            modify_other_keys: None,
            dec_ansi_mode: false,
            sixel_display_mode: false,
            use_private_color_registers_for_each_graphic: false,
            color_map,
            application_keypad: false,
            bracketed_paste: false,
            focus_tracking: false,
            mouse_encoding: MouseEncoding::X10,
            keyboard_encoding: KeyboardEncoding::Xterm,
            sixel_scrolls_right: false,
            any_event_mouse: false,
            button_event_mouse: false,
            mouse_tracking: false,
            last_mouse_move: None,
            cursor_visible: true,
            g0_charset: CharSet::Ascii,
            g1_charset: CharSet::Ascii,
            shift_out: false,
            newline_mode: false,
            current_mouse_buttons: vec![],
            tabs: TabStop::new(size.cols, 8),
            title: "wezterm".to_string(),
            icon_title: None,
            palette: None,
            pixel_height: size.pixel_height,
            pixel_width: size.pixel_width,
            dpi: size.dpi,
            clipboard: None,
            device_control_handler: None,
            alert_handler: None,
            download_handler: None,
            current_dir: None,
            term_program: term_program.to_string(),
            term_version: term_version.to_string(),
            writer,
            image_cache: lru::LruCache::new(NonZeroUsize::new(16).unwrap()),
            user_vars: HashMap::new(),
            kitty_img: Default::default(),
            seqno,
            unicode_version,
            unicode_version_stack: vec![],
            suppress_initial_title_change: false,
            enable_conpty_quirks: false,
            accumulating_title: None,
            lost_focus_seqno: seqno,
            lost_focus_alerted_seqno: seqno,
            focused: true,
            bidi_enabled: None,
            bidi_hint: None,
            progress: Progress::default(),
        }
    }

    pub fn enable_conpty_quirks(&mut self) {
        self.enable_conpty_quirks = true;
        self.suppress_initial_title_change = true;
    }

    pub fn current_seqno(&self) -> SequenceNo {
        self.seqno
    }

    pub fn increment_seqno(&mut self) {
        self.seqno += 1;
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

    pub fn set_download_handler(&mut self, handler: &Arc<dyn DownloadHandler>) {
        self.download_handler.replace(handler.clone());
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

    pub fn get_progress(&self) -> Progress {
        self.progress.clone()
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

    /// If the current overridden palette is effectively the same as
    /// the configured palette, remove the override and treat it as
    /// being the same as the configured state.
    /// This allows runtime changes to the configuration to take effect.
    pub fn implicit_palette_reset_if_same_as_configured(&mut self) {
        if self
            .palette
            .as_ref()
            .map(|p| *p == self.config.color_palette())
            .unwrap_or(false)
        {
            self.palette.take();
        }
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

    pub fn erase_scrollback_and_viewport(&mut self) {
        // Since we may be called outside of perform_actions,
        // we need to ensure that we increment the seqno in
        // order to correctly invalidate the display
        self.increment_seqno();
        self.erase_in_display(EraseInDisplay::EraseScrollback);

        let row_index = self.screen.phys_row(self.cursor.y);
        let rows = self.screen.lines_in_phys_range(row_index..row_index + 1);

        self.erase_in_display(EraseInDisplay::EraseDisplay);

        for (idx, row) in rows.into_iter().enumerate() {
            *self.screen.line_mut(idx) = row;
        }

        self.cursor.y = 0;
    }

    /// Discards the scrollback, leaving only the data that is present
    /// in the viewport.
    pub fn erase_scrollback(&mut self) {
        // Since we may be called outside of perform_actions,
        // we need to ensure that we increment the seqno in
        // order to correctly invalidate the display
        self.increment_seqno();
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
        if focused == self.focused {
            return;
        }
        if !focused {
            // notify app of release of buttons
            let buttons = self.current_mouse_buttons.clone();
            for b in buttons {
                self.mouse_event(MouseEvent {
                    kind: MouseEventKind::Release,
                    button: b,
                    modifiers: KeyModifiers::NONE,
                    x: 0,
                    y: 0,
                    x_pixel_offset: 0,
                    y_pixel_offset: 0,
                })
                .ok();
            }
        }
        if self.focus_tracking {
            write!(self.writer, "{}{}", CSI, if focused { "I" } else { "O" }).ok();
            self.writer.flush().ok();
        }
        self.focused = focused;
        if !focused {
            self.lost_focus_seqno = self.seqno;
        }
    }

    /// Returns true if there is new output since the terminal
    /// lost focus
    pub fn has_unseen_output(&self) -> bool {
        !self.focused && self.seqno > self.lost_focus_seqno
    }

    pub(crate) fn trigger_unseen_output_notif(&mut self) {
        if self.has_unseen_output() {
            // We want to avoid over-notifying about output events,
            // so here we gate the notification to the case where
            // we have lost the focus more recently than the last
            // time we notified about it
            if self.lost_focus_seqno > self.lost_focus_alerted_seqno {
                self.lost_focus_alerted_seqno = self.seqno;
                if let Some(handler) = self.alert_handler.as_mut() {
                    handler.alert(Alert::OutputSinceFocusLost);
                }
            }
        }
    }

    /// Send text to the terminal that is the result of pasting.
    /// If bracketed paste mode is enabled, the paste is enclosed
    /// in the bracketing, otherwise it is fed to the writer as-is.
    /// De-fang the text by removing any embedded bracketed paste
    /// sequence that may be present.
    pub fn send_paste(&mut self, text: &str) -> Result<(), Error> {
        let mut buf = String::new();
        if self.bracketed_paste {
            buf.push_str("\x1b[200~");
        }

        let canon = if self.bracketed_paste {
            NewlineCanon::None
        } else {
            self.config.canonicalize_pasted_newlines()
        };

        let canon = canon.canonicalize(text);
        let de_fanged = canon.replace("\x1b[200~", "").replace("\x1b[201~", "");
        buf.push_str(&de_fanged);

        if self.bracketed_paste {
            buf.push_str("\x1b[201~");
        }

        self.writer.write_all(buf.as_bytes())?;
        self.writer.flush()?;
        Ok(())
    }

    /// Informs the terminal that the viewport of the window has resized to the
    /// specified dimensions.
    /// We need to resize both the primary and alt screens, adjusting
    /// the cursor positions of both accordingly.
    pub fn resize(&mut self, size: TerminalSize) {
        self.increment_seqno();
        let (cursor_main, cursor_alt) = if self.screen.alt_screen_is_active {
            (
                self.screen
                    .saved_cursor
                    .as_ref()
                    .map(|s| s.position)
                    .unwrap_or_else(CursorPosition::default),
                self.cursor,
            )
        } else {
            (
                self.cursor,
                self.screen
                    .alt_saved_cursor
                    .as_ref()
                    .map(|s| s.position)
                    .unwrap_or_else(CursorPosition::default),
            )
        };

        let (adjusted_cursor_main, adjusted_cursor_alt) = self.screen.resize(
            size,
            cursor_main,
            cursor_alt,
            self.seqno,
            self.enable_conpty_quirks,
        );
        self.top_and_bottom_margins = 0..size.rows as i64;
        self.left_and_right_margins = 0..size.cols;
        self.pixel_height = size.pixel_height;
        self.pixel_width = size.pixel_width;
        self.dpi = size.dpi;
        self.tabs.resize(size.cols);

        if self.screen.alt_screen_is_active {
            self.set_cursor_pos(
                &Position::Absolute(adjusted_cursor_alt.x as i64),
                &Position::Absolute(adjusted_cursor_alt.y),
            );

            if let Some(saved) = self.screen.saved_cursor.as_mut() {
                saved.position.x = adjusted_cursor_main.x;
                saved.position.y = adjusted_cursor_main.y;
                saved.position.seqno = self.seqno;
                saved.wrap_next = false;
            }
        } else {
            self.set_cursor_pos(
                &Position::Absolute(adjusted_cursor_main.x as i64),
                &Position::Absolute(adjusted_cursor_main.y),
            );
            if let Some(saved) = self.screen.alt_saved_cursor.as_mut() {
                saved.position.x = adjusted_cursor_alt.x;
                saved.position.y = adjusted_cursor_alt.y;
                saved.position.seqno = self.seqno;
                saved.wrap_next = false;
            }
        }
    }

    pub fn get_size(&self) -> TerminalSize {
        let screen = self.screen();
        TerminalSize {
            dpi: self.dpi,
            pixel_width: self.pixel_width,
            pixel_height: self.pixel_height,
            rows: screen.physical_rows,
            cols: screen.physical_cols,
        }
    }

    fn palette_did_change(&mut self) {
        self.make_all_lines_dirty();
        if let Some(handler) = self.alert_handler.as_mut() {
            handler.alert(Alert::PaletteChanged);
        }
    }

    /// When dealing with selection, mark a range of lines as dirty
    pub fn make_all_lines_dirty(&mut self) {
        let seqno = self.seqno;
        let screen = self.screen_mut();
        screen.for_each_phys_line_mut(|_, line| {
            line.update_last_change_seqno(seqno);
        });
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
            seqno: self.cursor.seqno,
        }
    }

    /// Returns the current cell attributes of the screen
    pub fn pen(&self) -> CellAttributes {
        self.pen.clone()
    }

    pub fn user_vars(&self) -> &HashMap<String, String> {
        &self.user_vars
    }

    fn clear_semantic_attribute_due_to_movement(&mut self) {
        if self.clear_semantic_attribute_on_newline {
            self.clear_semantic_attribute_on_newline = false;
            self.pen.set_semantic_type(SemanticType::default());
        }
    }

    /// Sets the cursor position to precisely the x and values provided
    fn set_cursor_position_absolute(&mut self, x: usize, y: VisibleRowIndex) {
        if self.cursor.y != y {
            self.clear_semantic_attribute_due_to_movement();
        }
        self.cursor.y = y;
        self.cursor.x = x;
        self.cursor.seqno = self.seqno;
        self.wrap_next = false;
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
                        // We allow 1 extra for the cursor x position
                        // to account for some resize/rewrap scenarios
                        // where we don't want to forget that the
                        // cursor belongs to a wrapped line
                        self.screen().physical_cols + 1
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
        let seqno = self.seqno;
        let blank_attr = self.pen.clone_sgr_only();
        let top_and_bottom_margins = self.top_and_bottom_margins.clone();
        let left_and_right_margins = self.left_and_right_margins.clone();
        let bidi_mode = self.get_bidi_mode();
        self.screen_mut().scroll_up_within_margins(
            &top_and_bottom_margins,
            &left_and_right_margins,
            num_rows,
            seqno,
            blank_attr,
            bidi_mode,
        )
    }

    fn scroll_down(&mut self, num_rows: usize) {
        let seqno = self.seqno;
        let blank_attr = self.pen.clone_sgr_only();
        let top_and_bottom_margins = self.top_and_bottom_margins.clone();
        let left_and_right_margins = self.left_and_right_margins.clone();
        let bidi_mode = self.get_bidi_mode();
        self.screen_mut().scroll_down_within_margins(
            &top_and_bottom_margins,
            &left_and_right_margins,
            num_rows,
            seqno,
            blank_attr,
            bidi_mode,
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
        let seqno = self.seqno;
        let x = match self.tabs.find_next_tab_stop(self.cursor.x) {
            Some(x) => x,
            None => self.left_and_right_margins.end - 1,
        };
        self.cursor.x = x.min(self.left_and_right_margins.end - 1);
        self.cursor.seqno = seqno;
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

    /// <https://invisible-island.net/xterm/ctlseqs/ctlseqs.html#h4-Device-Control-functions:DCS-plus-q-Pt-ST.F95>
    /// XTGETTCAP
    fn xt_get_tcap(&mut self, names: Vec<String>) {
        let mut res = String::new();

        for name in &names {
            res.push_str("\x1bP");

            let encoded_name = hex::encode_upper(&name);
            match name.as_str() {
                "TN" | "name" => {
                    res.push_str("1+r");
                    res.push_str(&encoded_name);
                    res.push('=');

                    let encoded_val = hex::encode_upper(&self.term_program);
                    res.push_str(&encoded_val);
                }

                "Co" | "colors" => {
                    res.push_str("1+r");
                    res.push_str(&encoded_name);
                    res.push('=');
                    let encoded_val = hex::encode_upper("256");
                    res.push_str(&encoded_val);
                }

                "RGB" => {
                    res.push_str("1+r");
                    res.push_str(&encoded_name);
                    res.push('=');
                    let encoded_val = hex::encode_upper("8/8/8");
                    res.push_str(&encoded_val);
                }

                _ => {
                    if let Some(value) = DB.raw(name) {
                        res.push_str("1+r");
                        res.push_str(&encoded_name);
                        res.push('=');
                        let value = match value {
                            Value::True => hex::encode_upper("1"),
                            Value::Number(n) => hex::encode_upper(&n.to_string()),
                            Value::String(s) => hex::encode_upper(s),
                        };
                        res.push_str(&value);
                    } else {
                        log::trace!("xt_get_tcap: unknown name {}", name);
                        res.push_str("0+r");
                        res.push_str(&encoded_name);
                    }
                }
            }
            res.push_str("\x1b\\");
        }

        log::trace!(
            "XTGETTCAP {:?} responding with {}",
            names,
            res.escape_debug()
        );
        self.writer.write_all(res.as_bytes()).ok();
        self.writer.flush().ok();
    }

    fn perform_device(&mut self, dev: Device) {
        match dev {
            Device::DeviceAttributes(a) => {
                if self.config.log_unknown_escape_sequences() {
                    log::warn!("unhandled: {:?}", a);
                }
            }
            Device::SoftReset => {
                // TODO: see https://vt100.net/docs/vt510-rm/DECSTR.html
                self.pen = CellAttributes::default();
                self.insert = false;
                self.dec_origin_mode = false;
                // Note that xterm deviates from the documented DECSTR
                // setting for dec_auto_wrap, so we do too
                self.dec_auto_wrap = true;
                self.application_cursor_keys = false;
                self.modify_other_keys = None;
                self.application_keypad = false;
                self.top_and_bottom_margins = 0..self.screen().physical_rows as i64;
                self.left_and_right_margins = 0..self.screen().physical_cols;
                self.left_and_right_margin_mode = false;
                self.screen.activate_alt_screen(self.seqno);
                self.screen.saved_cursor().take();
                self.screen.activate_primary_screen(self.seqno);
                self.screen.saved_cursor().take();
                self.kitty_remove_all_placements(true);

                self.reverse_wraparound_mode = false;
                self.reverse_video_mode = false;
                self.bidi_enabled.take();
                self.bidi_hint.take();

                self.g0_charset = CharSet::Ascii;
                self.g1_charset = CharSet::Ascii;
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
                // Response is: Pp ; Pv ; Pc
                // Where Pp=1 means vt220
                // and Pv is the firmware version.
                // Pc is always 0.
                // Because our default TERM is xterm, the firmware
                // version will be considered to be equialent to xterm's
                // patch levels, with the following effects:
                // pv < 95 -> ttymouse=xterm
                // pv >= 95 < 277 -> ttymouse=xterm2
                // pv >= 277 -> ttymouse=sgr
                // pv >= 279 - xterm will probe for additional device settings.
                self.writer.write(b"\x1b[>1;277;0c").ok();
                self.writer.flush().ok();
            }
            Device::RequestTertiaryDeviceAttributes => {
                self.writer
                    .write(format!("\x1bP!|00000000{}", ST).as_bytes())
                    .ok();
                self.writer.flush().ok();
            }
            Device::RequestTerminalNameAndVersion => {
                self.writer.write(DCS.as_bytes()).ok();
                self.writer
                    .write(
                        format!(">|{} {}{}", self.term_program, self.term_version, ST).as_bytes(),
                    )
                    .ok();
                self.writer.flush().ok();
            }
            Device::RequestTerminalParameters(a) => {
                self.writer
                    .write(format!("\x1b[{};1;1;128;128;1;0x", a + 2).as_bytes())
                    .ok();
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

    /// Indicates that mode is permanently enabled
    fn decqrm_response_permanent(&mut self, mode: Mode) {
        let (is_dec, number) = match &mode {
            Mode::QueryDecPrivateMode(DecPrivateMode::Code(code)) => (true, code.to_u16().unwrap()),
            Mode::QueryDecPrivateMode(DecPrivateMode::Unspecified(code)) => (true, *code),
            Mode::QueryMode(TerminalMode::Code(code)) => (false, code.to_u16().unwrap()),
            Mode::QueryMode(TerminalMode::Unspecified(code)) => (false, *code),
            _ => unreachable!(),
        };

        let prefix = if is_dec { "?" } else { "" };

        write!(self.writer, "\x1b[{prefix}{number};3$y").ok();
        self.writer.flush().ok();
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
        self.writer.flush().ok();
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

            Mode::SetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::Win32InputMode)) => {
                self.keyboard_encoding = KeyboardEncoding::Win32;
            }

            Mode::ResetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::Win32InputMode)) => {
                self.keyboard_encoding = KeyboardEncoding::Xterm;
            }

            Mode::QueryDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::Win32InputMode)) => {
                self.decqrm_response(
                    mode,
                    true,
                    self.keyboard_encoding == KeyboardEncoding::Win32,
                );
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

            Mode::SetDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::GraphemeClustering,
            ))
            | Mode::ResetDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::GraphemeClustering,
            )) => {
                // Permanently enabled
            }

            Mode::QueryDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::GraphemeClustering,
            )) => {
                self.decqrm_response_permanent(mode);
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

            Mode::SetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::ReverseVideo)) => {
                // Turn on reverse video for all of the lines on the
                // display.
                self.reverse_video_mode = true;
            }

            Mode::ResetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::ReverseVideo)) => {
                // Turn off reverse video for all of the lines on the
                // display.
                self.reverse_video_mode = false;
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
            Mode::QueryDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::Select132Columns,
            )) => {
                self.decqrm_response(mode, true, false);
            }

            Mode::SetMode(TerminalMode::Code(TerminalModeCode::BiDirectionalSupportMode)) => {
                self.bidi_enabled.replace(true);
            }
            Mode::ResetMode(TerminalMode::Code(TerminalModeCode::BiDirectionalSupportMode)) => {
                self.bidi_enabled.replace(false);
            }
            Mode::QueryMode(TerminalMode::Code(TerminalModeCode::BiDirectionalSupportMode)) => {
                self.decqrm_response(
                    mode,
                    true,
                    self.bidi_enabled
                        .unwrap_or_else(|| self.config.bidi_mode().enabled),
                );
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

            Mode::SetMode(TerminalMode::Code(TerminalModeCode::AutomaticNewline)) => {
                self.newline_mode = true;
            }
            Mode::ResetMode(TerminalMode::Code(TerminalModeCode::AutomaticNewline)) => {
                self.newline_mode = false;
            }
            Mode::QueryMode(TerminalMode::Code(TerminalModeCode::AutomaticNewline)) => {
                self.decqrm_response(mode, true, self.newline_mode);
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
                    self.screen.activate_alt_screen(self.seqno);
                    self.pen = CellAttributes::default();
                }
            }
            Mode::ResetDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::OptEnableAlternateScreen,
            )) => {
                if self.screen.is_alt_screen_active() {
                    self.pen = CellAttributes::default();
                    self.erase_in_display(EraseInDisplay::EraseDisplay);
                    self.screen.activate_primary_screen(self.seqno);
                }
            }

            Mode::ResetDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::EnableAlternateScreen,
            )) => {
                if self.screen.is_alt_screen_active() {
                    self.screen.activate_primary_screen(self.seqno);
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

            Mode::SetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::SixelDisplayMode)) => {
                self.sixel_display_mode = true;
            }
            Mode::ResetDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::SixelDisplayMode,
            )) => {
                self.sixel_display_mode = false;
            }
            Mode::QueryDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::SixelDisplayMode,
            )) => {
                self.decqrm_response(mode, true, self.sixel_display_mode);
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
                self.mouse_encoding = MouseEncoding::SGR;
                self.last_mouse_move.take();
            }
            Mode::ResetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::SGRMouse)) => {
                self.mouse_encoding = MouseEncoding::X10;
                self.last_mouse_move.take();
            }
            Mode::QueryDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::SGRMouse)) => {
                self.decqrm_response(
                    mode,
                    true,
                    match self.mouse_encoding {
                        MouseEncoding::SGR => true,
                        _ => false,
                    },
                );
            }
            Mode::SetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::SGRPixelsMouse)) => {
                self.mouse_encoding = MouseEncoding::SgrPixels;
                self.last_mouse_move.take();
            }
            Mode::ResetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::SGRPixelsMouse)) => {
                self.mouse_encoding = MouseEncoding::X10;
                self.last_mouse_move.take();
            }
            Mode::QueryDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::SGRPixelsMouse)) => {
                self.decqrm_response(
                    mode,
                    true,
                    match self.mouse_encoding {
                        MouseEncoding::SgrPixels => true,
                        _ => false,
                    },
                );
            }

            Mode::SetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::Utf8Mouse)) => {
                self.mouse_encoding = MouseEncoding::Utf8;
                self.last_mouse_move.take();
            }
            Mode::ResetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::Utf8Mouse)) => {
                self.mouse_encoding = MouseEncoding::X10;
                self.last_mouse_move.take();
            }
            Mode::QueryDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::Utf8Mouse)) => {
                self.decqrm_response(
                    mode,
                    true,
                    match self.mouse_encoding {
                        MouseEncoding::Utf8 => true,
                        _ => false,
                    },
                );
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
                    self.screen.activate_alt_screen(self.seqno);
                    self.set_cursor_pos(&Position::Absolute(0), &Position::Absolute(0));
                    self.pen = CellAttributes::default();
                    self.erase_in_display(EraseInDisplay::EraseDisplay);
                }
            }
            Mode::ResetDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::ClearAndEnableAlternateScreen,
            )) => {
                if self.screen.is_alt_screen_active() {
                    self.screen.activate_primary_screen(self.seqno);
                    self.dec_restore_cursor();
                }
            }
            Mode::SaveDecPrivateMode(DecPrivateMode::Code(n))
            | Mode::RestoreDecPrivateMode(DecPrivateMode::Code(n)) => {
                log::warn!("save/restore dec mode {:?} unimplemented", n)
            }

            Mode::SetDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::MinTTYApplicationEscapeKeyMode,
            ))
            | Mode::ResetDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::MinTTYApplicationEscapeKeyMode,
            )) => {}

            Mode::SetDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::XTermMetaSendsEscape,
            ))
            | Mode::ResetDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::XTermMetaSendsEscape,
            )) => {}

            Mode::SetDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::XTermAltSendsEscape,
            ))
            | Mode::ResetDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::XTermAltSendsEscape,
            )) => {}

            Mode::SetDecPrivateMode(DecPrivateMode::Unspecified(_))
            | Mode::ResetDecPrivateMode(DecPrivateMode::Unspecified(_))
            | Mode::SaveDecPrivateMode(DecPrivateMode::Unspecified(_))
            | Mode::RestoreDecPrivateMode(DecPrivateMode::Unspecified(_)) => {
                if self.config.log_unknown_escape_sequences() {
                    log::warn!("unhandled DecPrivateMode {:?}", mode);
                }
            }

            mode @ Mode::SetMode(_) | mode @ Mode::ResetMode(_) => {
                if self.config.log_unknown_escape_sequences() {
                    log::warn!("unhandled {:?}", mode);
                }
            }

            Mode::XtermKeyMode {
                resource: XtermKeyModifierResource::OtherKeys,
                value,
            } => {
                self.modify_other_keys = match value {
                    Some(0) => None,
                    _ => value,
                };
                log::debug!("XtermKeyMode OtherKeys -> {:?}", self.modify_other_keys);
            }

            Mode::XtermKeyMode { resource, value } => {
                if self.config.log_unknown_escape_sequences() {
                    log::warn!("unhandled XtermKeyMode {:?} {:?}", resource, value);
                }
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
            for cell in line.visible_cells().skip(x_origin + left as usize) {
                if cell.cell_index() > x_origin + right as usize {
                    break;
                }

                let ch = cell.str().chars().next().unwrap() as u32;
                // debug!("y={} col={} ch={:x} cell={:?}", y + y_origin, col, ch, cell);

                checksum += u16::from(ch as u8);
            }
        }

        // Treat uninitialized cells as spaces.
        // The concept of uninitialized cells in wezterm is not the same as that on VT520 or that
        // on xterm, so, to prevent a lot of noise in esctest, treat them as spaces, at least when
        // asking for the checksum of a single cell (which is what esctest does).
        // See: https://github.com/wezterm/wezterm/pull/4565
        if checksum == 0 {
            32u16
        } else {
            checksum
        }
    }

    fn perform_csi_window(&mut self, window: Window) {
        match window {
            Window::ReportTextAreaSizeCells => {
                let screen = self.screen();
                let height = Some(screen.physical_rows as i64);
                let width = Some(screen.physical_cols as i64);

                let response = Box::new(Window::ResizeWindowCells { width, height });
                write!(self.writer, "{}", CSI::Window(response)).ok();
                self.writer.flush().ok();
            }

            Window::ReportCellSizePixels => {
                let screen = self.screen();
                let height = screen.physical_rows;
                let width = screen.physical_cols;
                let response = Box::new(Window::ReportCellSizePixelsResponse {
                    width: Some((self.pixel_width / width) as i64),
                    height: Some((self.pixel_height / height) as i64),
                });
                write!(self.writer, "{}", CSI::Window(response)).ok();
                self.writer.flush().ok();
            }

            Window::ReportTextAreaSizePixels => {
                let response = Box::new(Window::ResizeWindowPixels {
                    width: Some(self.pixel_width as i64),
                    height: Some(self.pixel_height as i64),
                });
                write!(self.writer, "{}", CSI::Window(response)).ok();
                self.writer.flush().ok();
            }

            Window::ReportWindowTitle => {
                if self.config.enable_title_reporting() {
                    write!(
                        self.writer,
                        "{}",
                        OperatingSystemCommand::SetWindowTitleSun(self.title.clone())
                    )
                    .ok();
                    self.writer.flush().ok();
                }
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

            _ => {
                if self.config.log_unknown_escape_sequences() {
                    log::warn!("unhandled Window CSI {:?}", window);
                }
            }
        }
    }

    fn erase_in_display(&mut self, erase: EraseInDisplay) {
        let seqno = self.seqno;
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
            let bidi_mode = self.get_bidi_mode();
            let screen = self.screen_mut();
            for y in row_range {
                screen.clear_line(y, col_range.clone(), &pen, seqno, bidi_mode);
                let line_idx = screen.phys_row(y);
                screen.line_mut(line_idx).set_single_width(seqno);
            }
        }
    }

    fn get_bidi_mode(&self) -> BidiMode {
        let mut mode = self.config.bidi_mode();
        if let Some(enabled) = &self.bidi_enabled {
            mode.enabled = *enabled;
        }
        if let Some(hint) = &self.bidi_hint {
            mode.hint = *hint;
        }
        mode
    }

    fn perform_csi_edit(&mut self, edit: Edit) {
        let seqno = self.seqno;
        match edit {
            Edit::DeleteCharacter(n) => {
                let y = self.cursor.y;
                let x = self.cursor.x;

                if x >= self.left_and_right_margins.start && x < self.left_and_right_margins.end {
                    let right_margin = self.left_and_right_margins.end;
                    let limit = (x + n as usize).min(right_margin);

                    let blank_attr = self.pen.clone_sgr_only();
                    let screen = self.screen_mut();
                    for _ in x..limit as usize {
                        screen.erase_cell(x, y, right_margin, seqno, blank_attr.clone());
                    }
                }
            }
            Edit::DeleteLine(n) => {
                if self.top_and_bottom_margins.contains(&self.cursor.y)
                    && self.left_and_right_margins.contains(&self.cursor.x)
                {
                    let top_and_bottom_margins = self.cursor.y..self.top_and_bottom_margins.end;
                    let left_and_right_margins = self.left_and_right_margins.clone();
                    let blank_attr = self.pen.clone_sgr_only();
                    let bidi_mode = self.get_bidi_mode();
                    self.screen_mut().scroll_up_within_margins(
                        &top_and_bottom_margins,
                        &left_and_right_margins,
                        n as usize,
                        seqno,
                        blank_attr,
                        bidi_mode,
                    );
                }
            }
            Edit::EraseCharacter(n) => {
                let y = self.cursor.y;
                let x = self.cursor.x;
                let limit = (x + n as usize).min(self.screen().physical_cols);
                {
                    let blank = Cell::blank_with_attrs(self.pen.clone_sgr_only());
                    let screen = self.screen_mut();
                    for x in x..limit as usize {
                        screen.set_cell(x, y, &blank, seqno);
                    }
                }
            }

            Edit::EraseInLine(erase) => {
                let cx = self.cursor.x;
                let cy = self.cursor.y;
                let pen = self.pen.clone_sgr_only();
                let cols = self.screen().physical_cols;
                let bidi_mode = self.get_bidi_mode();
                let range = match erase {
                    // If wrap_next is true, then cx is effectively 1 column to the right.
                    // It feels wrong to handle this here, but in trying to centralize
                    // the logic for updating the cursor position, it causes regressions
                    // in the test suite.
                    // So this is here for now until a better solution is found.
                    // <https://github.com/wezterm/wezterm/issues/3548>
                    EraseInLine::EraseToEndOfLine => cx + if self.wrap_next { 1 } else { 0 }..cols,
                    EraseInLine::EraseToStartOfLine => 0..cx + 1,
                    EraseInLine::EraseLine => 0..cols,
                };

                self.screen_mut()
                    .clear_line(cy, range, &pen, seqno, bidi_mode);
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
                        screen.insert_cell(x, y, margin, seqno);
                    }
                }
            }
            Edit::InsertLine(n) => {
                if self.top_and_bottom_margins.contains(&self.cursor.y)
                    && self.left_and_right_margins.contains(&self.cursor.x)
                {
                    let bidi_mode = self.get_bidi_mode();
                    let top_and_bottom_margins = self.cursor.y..self.top_and_bottom_margins.end;
                    let left_and_right_margins = self.left_and_right_margins.clone();
                    let blank_attr = self.pen.clone_sgr_only();
                    self.screen_mut().scroll_down_within_margins(
                        &top_and_bottom_margins,
                        &left_and_right_margins,
                        n as usize,
                        seqno,
                        blank_attr,
                        bidi_mode,
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

                    match line.cells_mut().get(to_copy).cloned() {
                        None => Cell::blank(),
                        Some(candidate) => {
                            if candidate.str() == " " && to_copy > 0 {
                                // It's a blank.  It may be the second part of
                                // a double-wide pair; look ahead of it.
                                let prior = &line.cells_mut()[to_copy - 1];
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

                        line.set_cell(x, cell.clone(), seqno);
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
            let cols = self.screen().physical_cols as u32;
            let left = left.as_zero_based().min(cols - 1).max(0) as usize;
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
        let seqno = self.seqno;
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
                self.tabs.clear(
                    to_clear,
                    self.cursor.x,
                    self.config.log_unknown_escape_sequences(),
                );
            }

            Cursor::TabulationControl(_) => {}
            Cursor::LineTabulation(_) => {}

            Cursor::Left(_n) => {
                // https://vt100.net/docs/vt510-rm/CUB.html
                unreachable!("Actually handled in Performer::csi_dispatch by rewriting as ControlCode::Backspace");
            }

            Cursor::Right(n) => {
                // https://vt100.net/docs/vt510-rm/CUF.html
                let cols = self.screen().physical_cols;
                let new_x = if self.cursor.x >= self.left_and_right_margins.end {
                    // outside the margin, so allow movement to screen edge
                    (self.cursor.x + n as usize).min(cols - 1)
                } else {
                    // Else constrain to margin
                    (self.cursor.x + n as usize).min(self.left_and_right_margins.end - 1)
                };

                self.cursor.x = new_x;
                self.cursor.seqno = seqno;
                self.wrap_next = false;
            }

            Cursor::Up(n) => {
                // https://vt100.net/docs/vt510-rm/CUU.html

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
                self.cursor.seqno = seqno;
                self.wrap_next = false;
            }
            Cursor::Down(n) => {
                // https://vt100.net/docs/vt510-rm/CUD.html
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
                self.cursor.seqno = seqno;
                self.wrap_next = false;
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
                self.cursor.seqno = seqno;
                self.wrap_next = false;
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
                self.cursor.seqno = seqno;
                self.wrap_next = false;
            }
            Cursor::PrecedingLine(n) => {
                // https://vt100.net/docs/vt510-rm/CPL.html
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
                self.cursor.seqno = seqno;
                self.wrap_next = false;
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
                    (self
                        .cursor
                        .x
                        .min(self.screen().physical_cols - 1)
                        .saturating_sub(if self.dec_origin_mode {
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
                g1_charset: CharSet::Ascii,
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
        self.newline_mode = false;
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
            Sgr::VerticalAlign(align) => {
                self.pen.set_vertical_align(align);
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
    pub fn get_semantic_zones(&mut self) -> anyhow::Result<Vec<SemanticZone>> {
        let screen = self.screen_mut();

        let mut current_zone: Option<SemanticZone> = None;
        let mut zones = vec![];

        let first_stable_row = screen.phys_to_stable_row_index(0);
        screen.for_each_phys_line_mut(|idx, line| {
            let stable_row = first_stable_row + idx as StableRowIndex;

            for zone_range in line.semantic_zone_ranges() {
                let new_zone = match current_zone.as_ref() {
                    None => true,
                    Some(zone) => zone.semantic_type != zone_range.semantic_type,
                };

                if new_zone {
                    if let Some(zone) = current_zone.take() {
                        zones.push(zone);
                    }

                    current_zone.replace(SemanticZone {
                        start_x: zone_range.range.start as usize,
                        start_y: stable_row,
                        end_x: zone_range.range.end as usize,
                        end_y: stable_row,
                        semantic_type: zone_range.semantic_type,
                    });
                }

                if let Some(zone) = current_zone.as_mut() {
                    zone.end_x = zone_range.range.end as usize;
                    zone.end_y = stable_row;
                }
            }
        });
        if let Some(zone) = current_zone.take() {
            zones.push(zone);
        }

        Ok(zones)
    }

    #[inline]
    pub fn get_reverse_video(&self) -> bool {
        self.reverse_video_mode
    }

    pub fn get_keyboard_encoding(&self) -> KeyboardEncoding {
        self.screen()
            .keyboard_stack
            .last()
            .copied()
            .unwrap_or(self.keyboard_encoding)
    }
}
