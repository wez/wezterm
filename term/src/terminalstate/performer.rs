use crate::terminal::{Alert, Progress};
use crate::terminalstate::{
    default_color_map, CharSet, MouseEncoding, TabStop, UnicodeVersionStackEntry,
};
use crate::{ClipboardSelection, Position, TerminalState, VisibleRowIndex, DCS, ST};
use finl_unicode::grapheme_clusters::Graphemes;
use log::{debug, error};
use num_traits::FromPrimitive;
use ordered_float::NotNan;
use std::fmt::Write;
use std::io::Write as _;
use std::ops::{Deref, DerefMut};
use termwiz::cell::{grapheme_column_width, Cell, CellAttributes, SemanticType};
use termwiz::escape::csi::{
    CharacterPath, EraseInDisplay, Keyboard, KittyKeyboardFlags, KittyKeyboardMode,
};
use termwiz::escape::osc::{
    ChangeColorPair, ColorOrQuery, FinalTermSemanticPrompt, ITermProprietary,
    ITermUnicodeVersionOp, Selection,
};
use termwiz::escape::{
    Action, ControlCode, DeviceControlMode, Esc, EscCode, OperatingSystemCommand, CSI,
};
use termwiz::input::KeyboardEncoding;
use unicode_normalization::{is_nfc_quick, IsNormalized, UnicodeNormalization};
use url::Url;
use wezterm_bidi::ParagraphDirectionHint;

/// A helper struct for implementing `vtparse::VTActor` while compartmentalizing
/// the terminal state and the embedding/host terminal interface
pub(crate) struct Performer<'a> {
    pub state: &'a mut TerminalState,
    print: String,
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
    pub fn new(state: &'a mut TerminalState) -> Self {
        Self {
            state,
            print: String::new(),
        }
    }

    /// Apply character set related remapping to the input glyph if required
    fn remap_grapheme<'b>(&self, g: &'b str) -> &'b str {
        if (self.shift_out && self.g1_charset == CharSet::DecLineDrawing)
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
        }
    }

    fn flush_print(&mut self) {
        if self.print.is_empty() {
            return;
        }

        let seqno = self.seqno;
        let mut p = std::mem::take(&mut self.print);
        let normalized: String;
        let text = if self.config.normalize_output_to_unicode_nfc()
            && is_nfc_quick(p.chars()) != IsNormalized::Yes
        {
            normalized = p.as_str().nfc().collect();
            normalized.as_str()
        } else {
            p.as_str()
        };

        for g in Graphemes::new(text) {
            let g = self.remap_grapheme(g);

            let print_width = grapheme_column_width(g, Some(self.unicode_version));
            if print_width == 0 {
                // We got a zero-width grapheme.
                // We used to force them into a cell to guarantee that we
                // preserved them in the model, but it introduces presentation
                // problems, such as <https://github.com/wezterm/wezterm/issues/1422>
                log::trace!("Eliding zero-width grapheme {:?}", g);
                continue;
            }

            if self.wrap_next {
                // Since we're implicitly moving the cursor to the next
                // line, we need to tag the current position as wrapped
                // so that we can correctly reflow it if the window is
                // resized.
                {
                    let y = self.cursor.y;
                    let is_conpty = self.state.enable_conpty_quirks;
                    let screen = self.screen_mut();
                    let y = screen.phys_row(y);

                    fn makes_sense_to_wrap(s: &str) -> bool {
                        let len = s.len();
                        match (len, s.chars().next()) {
                            (1, Some(c)) => c.is_alphanumeric() || c.is_ascii_punctuation(),
                            _ => true,
                        }
                    }

                    let should_mark_wrapped = !is_conpty
                        || screen
                            .line_mut(y)
                            .visible_cells()
                            .last()
                            .map(|cell| makes_sense_to_wrap(cell.str()))
                            .unwrap_or(false);
                    if should_mark_wrapped {
                        screen.line_mut(y).set_last_cell_was_wrapped(true, seqno);
                    }
                }
                self.new_line(true);
            }

            let x = self.cursor.x;
            let y = self.cursor.y;
            let width = self.left_and_right_margins.end;

            let pen = self.pen.clone();

            let wrappable = x + print_width >= width;

            if self.insert {
                let margin = self.left_and_right_margins.end;
                let screen = self.screen_mut();
                for _ in x..x + print_width as usize {
                    screen.insert_cell(x, y, margin, seqno);
                }
            }

            // Assign the cell
            log::trace!(
                "print x={} y={} print_width={} width={} cell={} {:?}",
                x,
                y,
                print_width,
                width,
                g,
                self.pen
            );
            self.screen_mut()
                .set_cell_grapheme(x, y, g, print_width, pen, seqno);

            if !wrappable {
                self.cursor.x += print_width;
                self.wrap_next = false;
            } else {
                self.wrap_next = self.dec_auto_wrap;
            }
        }

        std::mem::swap(&mut self.print, &mut p);
        self.print.clear();
    }

    /// ConPTY, at the time of writing, does something horrible to rewrite
    /// `ESC k TITLE ST` into something completely different and out-of-order,
    /// and critically, removes the ST.
    /// The result is that our hack to accumulate the tmux title gets stuck
    /// in a mode where all printable output is accumulated for the title.
    /// To combat this, we pop_tmux_title_state when we're obviously moving
    /// to different escape sequence parsing states.
    /// <https://github.com/wezterm/wezterm/issues/2442>
    fn pop_tmux_title_state(&mut self) {
        if let Some(title) = self.accumulating_title.take() {
            log::debug!("ST never received for pending tmux title escape sequence: {title:?}");
        }
    }

    pub fn perform(&mut self, action: Action) {
        debug!("perform {:?}", action);
        if self.suppress_initial_title_change {
            match &action {
                Action::OperatingSystemCommand(osc) => match **osc {
                    OperatingSystemCommand::SetIconNameAndWindowTitle(_) => {
                        debug!("suppressed {:?}", osc);
                        self.suppress_initial_title_change = false;
                        return;
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        match action {
            Action::Print(c) => self.print(c),
            Action::PrintString(s) => {
                for c in s.chars() {
                    self.print(c)
                }
            }
            Action::Control(code) => self.control(code),
            Action::DeviceControl(ctrl) => self.device_control(ctrl),
            Action::OperatingSystemCommand(osc) => self.osc_dispatch(*osc),
            Action::Esc(esc) => self.esc_dispatch(esc),
            Action::CSI(csi) => self.csi_dispatch(csi),
            Action::Sixel(sixel) => self.sixel(sixel),
            Action::XtGetTcap(names) => self.xt_get_tcap(names),
            Action::KittyImage(img) => {
                self.flush_print();
                if let Err(err) = self.kitty_img(*img) {
                    log::error!("kitty_img: {:#}", err);
                }
            }
        }
    }

    fn device_control(&mut self, ctrl: DeviceControlMode) {
        self.pop_tmux_title_state();
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
                                if self.config.log_unknown_escape_sequences() {
                                    log::warn!("unhandled DECRQSS {:?}", s);
                                }
                                // Reply that the request is invalid
                                write!(self.writer, "{}0$r{}", DCS, ST).ok();
                                self.writer.flush().ok();
                            }
                        }
                    }
                    _ => {
                        if self.config.log_unknown_escape_sequences() {
                            log::warn!("unhandled {:?}", s);
                        }
                    }
                }
            }
            _ => match self.device_control_handler.as_mut() {
                Some(handler) => handler.handle_device_control(ctrl),
                None => {
                    if self.config.log_unknown_escape_sequences() {
                        log::warn!("unhandled {:?}", ctrl);
                    }
                }
            },
        }
    }

    /// Draw a character to the screen
    fn print(&mut self, c: char) {
        // We buffer up the chars to increase the chances of correctly grouping graphemes into cells
        if let Some(title) = self.accumulating_title.as_mut() {
            title.push(c);
        } else {
            self.print.push(c);
        }
    }

    fn control(&mut self, control: ControlCode) {
        let seqno = self.seqno;
        self.pop_tmux_title_state();
        self.flush_print();
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
                    self.screen_mut().dirty_line(old_y, seqno);
                    self.screen_mut().dirty_line(y, seqno);
                    self.cursor.y = y;
                    self.wrap_next = false;
                }
                if self.newline_mode {
                    self.cursor.x = 0;
                    self.clear_semantic_attribute_due_to_movement();
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
                self.clear_semantic_attribute_due_to_movement();
                self.screen_mut().dirty_line(y, seqno);
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
                    // by 1 cancels the pending wrap.
                    self.wrap_next = false;
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

            ControlCode::Enquiry => {
                let response = self.config.enq_answerback();
                if response.len() > 0 {
                    write!(self.writer, "{}", response).ok();
                    self.writer.flush().ok();
                }
            }

            ControlCode::Null => {}

            _ => {
                if self.config.log_unknown_escape_sequences() {
                    log::warn!("unhandled ControlCode {:?}", control);
                }
            }
        }
    }

    fn csi_dispatch(&mut self, csi: CSI) {
        self.pop_tmux_title_state();
        self.flush_print();
        match csi {
            CSI::Sgr(sgr) => self.state.perform_csi_sgr(sgr),
            CSI::Cursor(termwiz::escape::csi::Cursor::Left(n)) => {
                // We treat CUB (Cursor::Left) the same as Backspace as
                // that is what xterm does.
                // <https://github.com/wezterm/wezterm/issues/1273>
                for _ in 0..n {
                    self.control(ControlCode::Backspace);
                }
            }
            CSI::Cursor(cursor) => self.state.perform_csi_cursor(cursor),
            CSI::Edit(edit) => self.state.perform_csi_edit(edit),
            CSI::Mode(mode) => self.state.perform_csi_mode(mode),
            CSI::Device(dev) => self.state.perform_device(*dev),
            CSI::Mouse(mouse) => error!("mouse report sent by app? {:?}", mouse),
            CSI::Window(window) => self.state.perform_csi_window(*window),
            CSI::SelectCharacterPath(CharacterPath::ImplementationDefault, _) => {
                self.state.bidi_hint.take();
            }
            CSI::SelectCharacterPath(CharacterPath::LeftToRightOrTopToBottom, _) => {
                self.state
                    .bidi_hint
                    .replace(ParagraphDirectionHint::LeftToRight);
            }
            CSI::SelectCharacterPath(CharacterPath::RightToLeftOrBottomToTop, _) => {
                self.state
                    .bidi_hint
                    .replace(ParagraphDirectionHint::RightToLeft);
            }
            CSI::Keyboard(Keyboard::SetKittyState { flags, mode }) => {
                if self.config.enable_kitty_keyboard() {
                    let current_flags = match self.screen().keyboard_stack.last() {
                        Some(KeyboardEncoding::Kitty(flags)) => *flags,
                        _ => KittyKeyboardFlags::NONE,
                    };
                    let flags = match mode {
                        KittyKeyboardMode::AssignAll => flags,
                        KittyKeyboardMode::SetSpecified => current_flags | flags,
                        KittyKeyboardMode::ClearSpecified => current_flags - flags,
                    };
                    self.screen_mut().keyboard_stack.pop();
                    self.screen_mut()
                        .keyboard_stack
                        .push(KeyboardEncoding::Kitty(flags));
                }
            }
            CSI::Keyboard(Keyboard::PushKittyState { flags, mode }) => {
                if self.config.enable_kitty_keyboard() {
                    let current_flags = match self.screen().keyboard_stack.last() {
                        Some(KeyboardEncoding::Kitty(flags)) => *flags,
                        _ => KittyKeyboardFlags::NONE,
                    };
                    let flags = match mode {
                        KittyKeyboardMode::AssignAll => flags,
                        KittyKeyboardMode::SetSpecified => current_flags | flags,
                        KittyKeyboardMode::ClearSpecified => current_flags - flags,
                    };
                    let screen = self.screen_mut();
                    screen.keyboard_stack.push(KeyboardEncoding::Kitty(flags));
                    if screen.keyboard_stack.len() > 128 {
                        screen.keyboard_stack.remove(0);
                    }
                }
            }
            CSI::Keyboard(Keyboard::PopKittyState(n)) => {
                for _ in 0..n {
                    self.screen_mut().keyboard_stack.pop();
                }
            }
            CSI::Keyboard(Keyboard::QueryKittySupport) => {
                if self.config.enable_kitty_keyboard() {
                    let flags = match self.screen().keyboard_stack.last() {
                        Some(KeyboardEncoding::Kitty(flags)) => *flags,
                        _ => KittyKeyboardFlags::NONE,
                    };
                    write!(self.writer, "\x1b[?{}u", flags.bits()).ok();
                    self.writer.flush().ok();
                }
            }
            CSI::Keyboard(Keyboard::ReportKittyState(_)) => {
                // This is a response to QueryKittySupport and it is invalid for us
                // to receive it. Just ignore it.
            }
            CSI::Unspecified(unspec) => {
                if self.config.log_unknown_escape_sequences() {
                    log::warn!("unknown unspecified CSI: {:?}", format!("{}", unspec));
                }
            }
        };
    }

    fn esc_dispatch(&mut self, esc: Esc) {
        let seqno = self.seqno;
        self.flush_print();
        if esc != Esc::Code(EscCode::StringTerminator) {
            self.pop_tmux_title_state();
        }
        match esc {
            Esc::Code(EscCode::StringTerminator) => {
                // String Terminator (ST); for the most part has nothing to do here, as its purpose is
                // handled implicitly through a state transition in the vtparse state tables.
                if let Some(title) = self.accumulating_title.take() {
                    self.osc_dispatch(OperatingSystemCommand::SetIconNameAndWindowTitle(title));
                }
            }
            Esc::Code(EscCode::TmuxTitle) => {
                self.accumulating_title.replace(String::new());
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

            Esc::Code(EscCode::DecDoubleHeightTopHalfLine) => {
                let idx = self.screen.phys_row(self.cursor.y);
                self.screen.line_mut(idx).set_double_height_top(seqno);
            }
            Esc::Code(EscCode::DecDoubleHeightBottomHalfLine) => {
                let idx = self.screen.phys_row(self.cursor.y);
                self.screen.line_mut(idx).set_double_height_bottom(seqno);
            }
            Esc::Code(EscCode::DecDoubleWidthLine) => {
                let idx = self.screen.phys_row(self.cursor.y);
                self.screen.line_mut(idx).set_double_width(seqno);
            }
            Esc::Code(EscCode::DecSingleWidthLine) => {
                let idx = self.screen.phys_row(self.cursor.y);
                self.screen.line_mut(idx).set_single_width(seqno);
            }

            Esc::Code(EscCode::DecScreenAlignmentDisplay) => {
                // This one is just to make vttest happy;
                // its original purpose was for aligning the CRT.
                // https://vt100.net/docs/vt510-rm/DECALN.html

                let screen = self.screen_mut();
                let col_range = 0..screen.physical_cols;
                for y in 0..screen.physical_rows as VisibleRowIndex {
                    let line_idx = screen.phys_row(y);
                    let line = screen.line_mut(line_idx);
                    line.resize(col_range.end, seqno);
                    line.fill_range(
                        col_range.clone(),
                        &Cell::new('E', CellAttributes::default()),
                        seqno,
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
                let seqno = self.seqno;
                self.pen = Default::default();
                self.cursor = Default::default();
                self.wrap_next = false;
                self.clear_semantic_attribute_on_newline = false;
                self.insert = false;
                self.dec_auto_wrap = true;
                self.reverse_wraparound_mode = false;
                self.reverse_video_mode = false;
                self.dec_origin_mode = false;
                self.use_private_color_registers_for_each_graphic = false;
                self.color_map = default_color_map();
                self.application_cursor_keys = false;
                self.sixel_display_mode = false;
                self.dec_ansi_mode = false;
                self.application_keypad = false;
                self.bracketed_paste = false;
                self.focus_tracking = false;
                self.mouse_tracking = false;
                self.mouse_encoding = MouseEncoding::X10;
                self.keyboard_encoding = KeyboardEncoding::Xterm;
                self.sixel_scrolls_right = false;
                self.any_event_mouse = false;
                self.button_event_mouse = false;
                self.current_mouse_buttons.clear();
                self.cursor_visible = true;
                self.g0_charset = CharSet::Ascii;
                self.g1_charset = CharSet::Ascii;
                self.shift_out = false;
                self.newline_mode = false;
                self.tabs = TabStop::new(self.screen().physical_cols, 8);
                self.palette.take();
                self.top_and_bottom_margins = 0..self.screen().physical_rows as VisibleRowIndex;
                self.left_and_right_margins = 0..self.screen().physical_cols;
                self.unicode_version = self.config.unicode_version();
                self.unicode_version_stack.clear();
                self.suppress_initial_title_change = false;
                self.accumulating_title.take();
                self.progress = Progress::default();

                self.screen.full_reset();
                self.screen.activate_alt_screen(seqno);
                self.erase_in_display(EraseInDisplay::EraseDisplay);
                self.screen.activate_primary_screen(seqno);
                self.erase_in_display(EraseInDisplay::EraseScrollback);
                self.erase_in_display(EraseInDisplay::EraseDisplay);
                self.palette_did_change();
            }

            _ => {
                if self.config.log_unknown_escape_sequences() {
                    log::warn!("ESC: unhandled {:?}", esc);
                }
            }
        }
    }

    fn osc_dispatch(&mut self, osc: OperatingSystemCommand) {
        self.pop_tmux_title_state();
        self.flush_print();
        match osc {
            OperatingSystemCommand::SetIconNameSun(title)
            | OperatingSystemCommand::SetIconName(title) => {
                if title.is_empty() {
                    self.icon_title = None;
                } else {
                    self.icon_title = Some(title);
                }
                let title = self.icon_title.clone();
                if let Some(handler) = self.alert_handler.as_mut() {
                    handler.alert(Alert::IconTitleChanged(title));
                }
            }
            OperatingSystemCommand::SetIconNameAndWindowTitle(title) => {
                self.icon_title.take();
                self.title = title.clone();
                if let Some(handler) = self.alert_handler.as_mut() {
                    handler.alert(Alert::WindowTitleChanged(title.clone()));
                    handler.alert(Alert::IconTitleChanged(Some(title)));
                }
            }

            OperatingSystemCommand::SetWindowTitleSun(title)
            | OperatingSystemCommand::SetWindowTitle(title) => {
                self.title = title.clone();
                if let Some(handler) = self.alert_handler.as_mut() {
                    handler.alert(Alert::WindowTitleChanged(title));
                }
            }
            OperatingSystemCommand::SetHyperlink(link) => {
                self.set_hyperlink(link);
            }
            OperatingSystemCommand::Unspecified(unspec) => {
                if self.config.log_unknown_escape_sequences() {
                    let mut output = String::new();
                    write!(&mut output, "Unhandled OSC ").ok();

                    for item in unspec {
                        write!(&mut output, " {}", String::from_utf8_lossy(&item)).ok();
                    }
                    log::warn!("{}", output);
                }
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
                    Err(err) => error!("failed to set clipboard in response to OSC 52: {:#?}", err),
                }
            }
            OperatingSystemCommand::ITermProprietary(iterm) => match iterm {
                ITermProprietary::RequestCellSize => {
                    let screen = self.screen();
                    let height = screen.physical_rows;
                    let width = screen.physical_cols;

                    let scale = if screen.dpi == 0 {
                        1.0
                    } else {
                        // Since iTerm2 is a macOS specific piece
                        // of software, it uses the macOS default dpi
                        // if 72 for the basis of its scale, regardless
                        // of the host base dpi.
                        screen.dpi as f32 / 72.
                    };
                    let width = (self.pixel_width as f32 / width as f32) / scale;
                    let height = (self.pixel_height as f32 / height as f32) / scale;

                    let response = OperatingSystemCommand::ITermProprietary(
                        ITermProprietary::ReportCellSize {
                            width_pixels: NotNan::new(width).unwrap(),
                            height_pixels: NotNan::new(height).unwrap(),
                            scale: if screen.dpi == 0 {
                                None
                            } else {
                                Some(NotNan::new(scale).unwrap())
                            },
                        },
                    );
                    write!(self.writer, "{}", response).ok();
                    self.writer.flush().ok();
                }
                ITermProprietary::File(image) => self.set_image(*image),
                ITermProprietary::SetUserVar { name, value } => {
                    self.user_vars.insert(name.clone(), value.clone());
                    if let Some(handler) = self.alert_handler.as_mut() {
                        handler.alert(Alert::SetUserVar { name, value });
                    }
                }
                ITermProprietary::UnicodeVersion(ITermUnicodeVersionOp::Set(n)) => {
                    self.unicode_version.version = n;
                }
                ITermProprietary::UnicodeVersion(ITermUnicodeVersionOp::Push(label)) => {
                    let vers = self.unicode_version;
                    self.unicode_version_stack
                        .push(UnicodeVersionStackEntry { vers, label });
                }
                ITermProprietary::UnicodeVersion(ITermUnicodeVersionOp::Pop(None)) => {
                    if let Some(entry) = self.unicode_version_stack.pop() {
                        self.unicode_version = entry.vers;
                    }
                }
                ITermProprietary::UnicodeVersion(ITermUnicodeVersionOp::Pop(Some(label))) => {
                    while let Some(entry) = self.unicode_version_stack.pop() {
                        self.unicode_version = entry.vers;
                        if entry.label.as_deref() == Some(&label) {
                            break;
                        }
                    }
                }
                _ => {
                    if self.config.log_unknown_escape_sequences() {
                        log::warn!("unhandled iterm2: {:?}", iterm);
                    }
                }
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
                FinalTermSemanticPrompt::MarkEndOfPromptAndStartOfInputUntilEndOfLine { .. },
            ) => {
                self.pen.set_semantic_type(SemanticType::Input);
                self.clear_semantic_attribute_on_newline = true;
            }
            OperatingSystemCommand::FinalTermSemanticPrompt(
                FinalTermSemanticPrompt::MarkEndOfInputAndStartOfOutput { .. },
            ) => {
                self.pen.set_semantic_type(SemanticType::Output);
            }

            OperatingSystemCommand::FinalTermSemanticPrompt(
                FinalTermSemanticPrompt::CommandStatus { .. },
            ) => {}

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
                    handler.alert(Alert::CurrentWorkingDirectoryChanged);
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
                self.implicit_palette_reset_if_same_as_configured();
                self.palette_did_change();
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
                self.implicit_palette_reset_if_same_as_configured();
                self.palette_did_change();
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
                                            vec![ColorOrQuery::Color(self.palette().$name.into())],
                                        );
                                        log::trace!("Color Query response {:?}", response);
                                        write!(self.writer, "{}", response).ok();
                                        self.writer.flush().ok();
                                    }
                                    ColorOrQuery::Color(c) => self.palette_mut().$name = c.into(),
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
                                    self.palette_mut().cursor_border = c.into();
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
                self.implicit_palette_reset_if_same_as_configured();
                self.palette_did_change();
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
                self.implicit_palette_reset_if_same_as_configured();
                self.palette_did_change();
            }
            OperatingSystemCommand::ConEmuProgress(prog) => {
                use termwiz::escape::osc::Progress as TProg;
                let prog = match prog {
                    TProg::None => Progress::None,
                    TProg::SetPercentage(p) => Progress::Percentage(p),
                    TProg::SetError(p) => Progress::Error(p),
                    TProg::SetIndeterminate => Progress::Indeterminate,
                    TProg::Paused => Progress::None,
                };
                if prog != self.progress {
                    self.progress = prog.clone();
                    if let Some(handler) = self.alert_handler.as_mut() {
                        handler.alert(Alert::Progress(prog));
                    }
                }
            }
        }
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
