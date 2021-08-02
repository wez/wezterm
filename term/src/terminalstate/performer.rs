use crate::input::MouseButton;
use crate::terminal::Alert;
use crate::terminalstate::{default_color_map, CharSet, TabStop};
use crate::{ClipboardSelection, Position, TerminalState, VisibleRowIndex};
use crate::{DCS, ST};
use log::{debug, error};
use num_traits::FromPrimitive;
use std::fmt::Write;
use std::ops::{Deref, DerefMut};
use termwiz::cell::{unicode_column_width, Cell, CellAttributes, SemanticType};
use termwiz::escape::csi::EraseInDisplay;
use termwiz::escape::osc::{
    ChangeColorPair, ColorOrQuery, FinalTermSemanticPrompt, ITermProprietary, Selection,
};
use termwiz::escape::{
    Action, ControlCode, DeviceControlMode, Esc, EscCode, OperatingSystemCommand, CSI,
};
use url::Url;

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
        self.flush_print();
    }
}

impl<'a> Performer<'a> {
    pub fn new(state: &'a mut TerminalState) -> Self {
        Self { state, print: None }
    }

    fn flush_print(&mut self) {
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
                // Since we're implicitly moving the cursor to the next
                // line, we need to tag the current position as wrapped
                // so that we can correctly reflow it if the window is
                // resized.
                {
                    let x = self.cursor.x;
                    let y = self.cursor.y;
                    let screen = self.screen_mut();
                    if let Some(cell) = screen.cell_mut(x, y) {
                        cell.attrs_mut().set_wrapped(true);
                    }
                }
                self.new_line(true);
            }

            let x = self.cursor.x;
            let y = self.cursor.y;
            let width = self.left_and_right_margins.end;

            let pen = self.pen.clone();
            // the max(1) here is to ensure that we advance to the next cell
            // position for zero-width graphemes.  We want to make sure that
            // they occupy a cell so that we can re-emit them when we output them.
            // If we didn't do this, then we'd effectively filter them out from
            // the model, which seems like a lossy design choice.
            let print_width = unicode_column_width(g).max(1);
            let is_last = graphemes.peek().is_none();
            let wrappable = x + print_width >= width;

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
                "print x={} y={} is_last={} print_width={} width={} cell={:?}",
                x,
                y,
                is_last,
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
            Action::XtGetTcap(names) => self.xt_get_tcap(names),
            Action::KittyImage(img) => {
                self.flush_print();
                if let Err(err) = self.kitty_img(img) {
                    log::error!("kitty_img: {:#}", err);
                }
            }
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
                    self.screen_mut().dirty_line(old_y);
                    self.screen_mut().dirty_line(y);
                    self.cursor.y = y;
                    self.wrap_next = false;
                }
                if self.newline_mode {
                    self.cursor.x = 0;
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

            ControlCode::Enquiry => {
                let response = self.config.enq_answerback();
                if response.len() > 0 {
                    write!(self.writer, "{}", response).ok();
                    self.writer.flush().ok();
                }
            }

            _ => log::warn!("unhandled ControlCode {:?}", control),
        }
    }

    fn csi_dispatch(&mut self, csi: CSI) {
        self.flush_print();
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
                self.screen.line_mut(idx).set_double_height_top();
            }
            Esc::Code(EscCode::DecDoubleHeightBottomHalfLine) => {
                let idx = self.screen.phys_row(self.cursor.y);
                self.screen.line_mut(idx).set_double_height_bottom();
            }
            Esc::Code(EscCode::DecDoubleWidthLine) => {
                let idx = self.screen.phys_row(self.cursor.y);
                self.screen.line_mut(idx).set_double_width();
            }
            Esc::Code(EscCode::DecSingleWidthLine) => {
                let idx = self.screen.phys_row(self.cursor.y);
                self.screen.line_mut(idx).set_single_width();
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
                self.reverse_video_mode = false;
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
                self.newline_mode = false;
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
        self.flush_print();
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
