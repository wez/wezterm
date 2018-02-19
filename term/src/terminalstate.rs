use super::*;

struct TabStop {
    tabs: Vec<bool>,
}

impl TabStop {
    fn new(screen_width: usize, tab_width: usize) -> Self {
        let mut tabs = Vec::with_capacity(screen_width);

        for i in 0..screen_width {
            tabs.push((i % tab_width) == 0);
        }
        Self { tabs }
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
}

pub struct TerminalState {
    /// The primary screen + scrollback
    screen: Screen,
    /// The alternate screen; no scrollback
    alt_screen: Screen,
    /// Tells us which screen is active
    alt_screen_is_active: bool,
    /// The current set of attributes in effect for the next
    /// attempt to print to the display
    pen: CellAttributes,
    /// The current cursor position, relative to the top left
    /// of the screen.  0-based index.
    cursor: CursorPosition,
    saved_cursor: CursorPosition,

    /// if true, implicitly move to the next line on the next
    /// printed character
    wrap_next: bool,

    /// Some parsing operations may yield responses that need
    /// to be returned to the client.  They are collected here
    /// and this is used as the result of the advance_bytes()
    /// method.
    answerback: Vec<AnswerBack>,

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

    sgr_mouse: bool,
    button_event_mouse: bool,
    current_mouse_button: MouseButton,
    mouse_position: CursorPosition,
    cursor_visible: bool,

    /// Which hyperlink is considered to be highlighted, because the
    /// mouse_position is over a cell with a Hyperlink attribute.
    current_highlight: Option<Rc<Hyperlink>>,

    /// Keeps track of double and triple clicks
    last_mouse_click: Option<LastMouseClick>,

    /// Used to compute the offset to the top of the viewport.
    /// This is used to display the scrollback of the terminal.
    /// It is distinct from the scroll_region in that the scroll region
    /// afects how the terminal output is scrolled as data is output,
    /// and the viewport_offset is used to index into the scrollback
    /// purely for display purposes.
    /// The offset is measured from the top of the physical viewable
    /// screen with larger numbers going backwards.
    viewport_offset: VisibleRowIndex,

    /// Remembers the starting coordinate of the selection prior to
    /// dragging.
    selection_start: Option<SelectionCoordinate>,
    /// Holds the not-normalized selection range.
    selection_range: Option<SelectionRange>,

    tabs: TabStop,
}

impl TerminalState {
    pub fn new(
        physical_rows: usize,
        physical_cols: usize,
        scrollback_size: usize,
    ) -> TerminalState {
        let screen = Screen::new(physical_rows, physical_cols, scrollback_size);
        let alt_screen = Screen::new(physical_rows, physical_cols, 0);

        TerminalState {
            screen,
            alt_screen,
            alt_screen_is_active: false,
            pen: CellAttributes::default(),
            cursor: CursorPosition::default(),
            saved_cursor: CursorPosition::default(),
            answerback: Vec::new(),
            scroll_region: 0..physical_rows as VisibleRowIndex,
            wrap_next: false,
            application_cursor_keys: false,
            application_keypad: false,
            bracketed_paste: false,
            sgr_mouse: false,
            button_event_mouse: false,
            cursor_visible: true,
            current_mouse_button: MouseButton::None,
            mouse_position: CursorPosition::default(),
            current_highlight: None,
            last_mouse_click: None,
            viewport_offset: 0,
            selection_range: None,
            selection_start: None,
            tabs: TabStop::new(physical_cols, 8),
        }
    }

    pub fn screen(&self) -> &Screen {
        if self.alt_screen_is_active {
            &self.alt_screen
        } else {
            &self.screen
        }
    }

    pub fn screen_mut(&mut self) -> &mut Screen {
        if self.alt_screen_is_active {
            &mut self.alt_screen
        } else {
            &mut self.screen
        }
    }

    pub fn get_selection_text(&self) -> String {
        let mut s = String::new();

        if let Some(sel) = self.selection_range.as_ref().map(|r| r.normalize()) {
            let screen = self.screen();
            for y in sel.rows() {
                let idx = screen.scrollback_or_visible_row(y);
                let cols = sel.cols_for_row(y);
                if s.len() > 0 {
                    s.push('\n');
                }
                s.push_str(&screen.lines[idx].columns_as_str(cols).trim_right());
            }
        }

        s
    }

    /// Dirty the lines in the current selection range
    fn dirty_selection_lines(&mut self) {
        if let Some(sel) = self.selection_range.as_ref().map(|r| r.normalize()) {
            let screen = self.screen_mut();
            for y in screen.scrollback_or_visible_range(&sel.rows()) {
                screen.line_mut(y).set_dirty();
            }
        }
    }

    pub fn clear_selection(&mut self) {
        self.dirty_selection_lines();
        self.selection_range = None;
        self.selection_start = None;
    }

    fn hyperlink_for_cell(
        &self,
        x: usize,
        y: ScrollbackOrVisibleRowIndex,
    ) -> Option<Rc<Hyperlink>> {
        let screen = self.screen();
        let idx = screen.scrollback_or_visible_row(y);
        let line = match &screen.lines.get(idx) {
            &Some(line) => line,
            &None => return None,
        };
        match line.cells.get(x) {
            Some(cell) => cell.attrs.hyperlink.as_ref().cloned(),
            None => None,
        }
    }

    /// Invalidate rows that have hyperlinks
    fn invalidate_hyperlinks(&mut self) {
        let screen = self.screen_mut();
        for line in screen.lines.iter_mut() {
            if line.has_hyperlink() {
                line.set_dirty();
            }
        }
    }

    /// Called after a mouse move or viewport scroll to recompute the
    /// current highlight
    fn recompute_highlight(&mut self) {
        self.current_highlight = self.hyperlink_for_cell(
            self.mouse_position.x,
            self.mouse_position.y as ScrollbackOrVisibleRowIndex -
                self.viewport_offset as ScrollbackOrVisibleRowIndex,
        );
        self.invalidate_hyperlinks();
    }

    pub fn mouse_event(
        &mut self,
        mut event: MouseEvent,
        host: &mut TerminalHost,
    ) -> Result<(), Error> {
        // Clamp the mouse coordinates to the size of the model.
        // This situation can trigger for example when the
        // window is resized and leaves a partial row at the bottom of the
        // terminal.  The mouse can move over that portion and the gui layer
        // can thus send us out-of-bounds row or column numbers.  We want to
        // make sure that we clamp this and handle it nicely at the model layer.
        event.y = event.y.min(self.screen().physical_rows as i64 - 1);
        event.x = event.x.min(self.screen().physical_cols - 1);

        // Remember the last reported mouse position so that we can use it
        // for highlighting clickable things elsewhere.
        let new_position = CursorPosition {
            x: event.x,
            y: event.y as VisibleRowIndex,
        };

        if new_position != self.mouse_position {
            self.mouse_position = new_position;
            self.recompute_highlight();
        }

        // First pass to figure out if we're messing with the selection
        let send_event = self.sgr_mouse && !event.modifiers.contains(KeyModifiers::SHIFT);

        // Perform click counting
        if event.kind == MouseEventKind::Press {
            let click = match self.last_mouse_click.take() {
                None => LastMouseClick::new(event.button),
                Some(click) => click.add(event.button),
            };
            self.last_mouse_click = Some(click);
        }

        if !send_event {
            match (event, self.current_mouse_button) {
                (MouseEvent {
                     kind: MouseEventKind::Press,
                     button: MouseButton::Left,
                     ..
                 },
                 _) => {
                    self.current_mouse_button = MouseButton::Left;
                    self.dirty_selection_lines();
                    match self.last_mouse_click.as_ref() {
                        // Single click prepares the start of a new selection
                        Some(&LastMouseClick { streak: 1, .. }) => {
                            // Prepare to start a new selection.
                            // We don't form the selection until the mouse drags.
                            self.selection_range = None;
                            self.selection_start = Some(SelectionCoordinate {
                                x: event.x,
                                y: event.y as ScrollbackOrVisibleRowIndex,
                            });
                            host.set_clipboard(None)?;
                        }
                        // Double click to select a word on the current line
                        Some(&LastMouseClick { streak: 2, .. }) => {
                            let y = event.y as ScrollbackOrVisibleRowIndex;
                            let idx = self.screen().scrollback_or_visible_row(y);
                            let line = self.screen().lines[idx].as_str();
                            use unicode_segmentation::UnicodeSegmentation;

                            self.selection_start = None;
                            self.selection_range = None;
                            // TODO: allow user to configure the word boundary rules.
                            // Also consider making the default work with URLs?
                            for (x, word) in line.split_word_bound_indices() {
                                if event.x < x {
                                    break;
                                }
                                if event.x <= x + word.len() {
                                    // this is our word
                                    let start = SelectionCoordinate { x, y };
                                    let end = SelectionCoordinate {
                                        x: x + word.len() - 1,
                                        y,
                                    };
                                    self.selection_start = Some(start.clone());
                                    self.selection_range = Some(SelectionRange { start, end });
                                    self.dirty_selection_lines();
                                    let text = self.get_selection_text();
                                    debug!(
                                        "finish 2click selection {:?} '{}'",
                                        self.selection_range,
                                        text
                                    );
                                    host.set_clipboard(Some(text))?;
                                    return Ok(());
                                }
                            }
                            host.set_clipboard(None)?;
                        }
                        // triple click to select the current line
                        Some(&LastMouseClick { streak: 3, .. }) => {
                            self.selection_start = Some(SelectionCoordinate {
                                x: event.x,
                                y: event.y as ScrollbackOrVisibleRowIndex,
                            });
                            self.selection_range = Some(SelectionRange {
                                start: SelectionCoordinate {
                                    x: 0,
                                    y: event.y as ScrollbackOrVisibleRowIndex,
                                },
                                end: SelectionCoordinate {
                                    x: usize::max_value(),
                                    y: event.y as ScrollbackOrVisibleRowIndex,
                                },
                            });
                            self.dirty_selection_lines();
                            let text = self.get_selection_text();
                            debug!(
                                "finish 3click selection {:?} '{}'",
                                self.selection_range,
                                text
                            );
                            host.set_clipboard(Some(text))?;
                        }
                        // otherwise, clear out the selection
                        _ => {
                            self.selection_range = None;
                            self.selection_start = None;
                            host.set_clipboard(None)?;
                        }
                    }

                    return Ok(());
                }
                (MouseEvent {
                     kind: MouseEventKind::Release,
                     button: MouseButton::Left,
                     ..
                 },
                 _) => {
                    // Finish selecting a region, update clipboard
                    self.current_mouse_button = MouseButton::None;
                    match self.last_mouse_click.as_ref() {
                        // Only consider a drag selection if we have a streak==1.
                        // The double/triple click cases are handled above.
                        Some(&LastMouseClick { streak: 1, .. }) => {
                            let text = self.get_selection_text();
                            if text.len() > 0 {
                                debug!(
                                    "finish drag selection {:?} '{}'",
                                    self.selection_range,
                                    text
                                );
                                host.set_clipboard(Some(text))?;
                            } else {
                                // If the button release wasn't a drag, consider
                                // whether it was a click on a hyperlink
                                if let Some(link) = self.current_highlight() {
                                    host.click_link(&link);
                                }
                            }
                            return Ok(());
                        }
                        _ => {}
                    }
                }
                (MouseEvent { kind: MouseEventKind::Move, .. }, MouseButton::Left) => {
                    // dragging out the selection region
                    // TODO: may drag and change the viewport
                    self.dirty_selection_lines();
                    let end = SelectionCoordinate {
                        x: event.x,
                        y: event.y as ScrollbackOrVisibleRowIndex,
                    };
                    let sel = match self.selection_range.take() {
                        None => {
                            SelectionRange::start(self.selection_start.unwrap_or(end.clone()))
                                .extend(end)
                        }
                        Some(sel) => sel.extend(end),
                    };
                    self.selection_range = Some(sel);
                    // Dirty lines again to reflect new range
                    self.dirty_selection_lines();
                    return Ok(());
                }
                _ => {}
            }
        }

        match event {
            MouseEvent {
                kind: MouseEventKind::Press,
                button: MouseButton::WheelUp,
                ..
            } |
            MouseEvent {
                kind: MouseEventKind::Press,
                button: MouseButton::WheelDown,
                ..
            } => {
                let (report_button, scroll_delta, key) = if event.button == MouseButton::WheelUp {
                    (64, -1, KeyCode::Up)
                } else {
                    (65, 1, KeyCode::Down)
                };

                if self.sgr_mouse {
                    write!(
                        host.writer(),
                        "\x1b[<{};{};{}M",
                        report_button,
                        event.x + 1,
                        event.y + 1
                    )?;
                } else if self.alt_screen_is_active {
                    // Send cursor keys instead (equivalent to xterm's alternateScroll mode)
                    self.key_down(key, KeyModifiers::default(), host)?;
                } else {
                    self.scroll_viewport(scroll_delta)
                }
            }
            MouseEvent { kind: MouseEventKind::Press, .. } => {
                self.current_mouse_button = event.button;
                if let Some(button) = match event.button {
                    MouseButton::Left => Some(0),
                    MouseButton::Middle => Some(1),
                    MouseButton::Right => Some(2),
                    _ => None,
                }
                {
                    if self.sgr_mouse {
                        write!(
                            host.writer(),
                            "\x1b[<{};{};{}M",
                            button,
                            event.x + 1,
                            event.y + 1
                        )?;
                    } else if event.button == MouseButton::Middle {
                        let clip = host.get_clipboard()?;
                        if self.bracketed_paste {
                            write!(host.writer(), "\x1b[200~{}\x1b[201~", clip)?;
                        } else {
                            write!(host.writer(), "{}", clip)?;
                        }
                    }
                }
            }
            MouseEvent { kind: MouseEventKind::Release, .. } => {
                self.current_mouse_button = MouseButton::None;
                if self.sgr_mouse {
                    write!(host.writer(), "\x1b[<3;{};{}m", event.x + 1, event.y + 1)?;
                }
            }
            MouseEvent { kind: MouseEventKind::Move, .. } => {
                if let Some(button) = match (self.current_mouse_button, self.button_event_mouse) {
                    (_, false) => None,
                    (MouseButton::Left, true) => Some(32),
                    (MouseButton::Middle, true) => Some(33),
                    (MouseButton::Right, true) => Some(34),
                    (..) => None,
                }
                {
                    if self.sgr_mouse {
                        write!(
                            host.writer(),
                            "\x1b[<{};{};{}M",
                            button,
                            event.x + 1,
                            event.y + 1
                        )?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Processes a key_down event generated by the gui/render layer
    /// that is embedding the Terminal.  This method translates the
    /// keycode into a sequence of bytes to send to the slave end
    /// of the pty via the `Write`-able object provided by the caller.
    pub fn key_down(
        &mut self,
        key: KeyCode,
        mods: KeyModifiers,
        host: &mut TerminalHost,
    ) -> Result<(), Error> {
        const CTRL: KeyModifiers = KeyModifiers::CTRL;
        const SHIFT: KeyModifiers = KeyModifiers::SHIFT;
        const ALT: KeyModifiers = KeyModifiers::ALT;
        const APPCURSOR: bool = true;
        use KeyCode::*;

        let ctrl = mods & CTRL;
        let shift = mods & SHIFT;
        let alt = mods & ALT;

        let mut buf = String::new();

        // TODO: also respect self.application_keypad

        let to_send = match (key, ctrl, alt, shift, self.application_cursor_keys) {
            (Char(c), CTRL, _, SHIFT, _) if c <= 0xff as char => {
                // If shift is held we have C == 0x43 and want to translate
                // that into 0x03
                buf.push((c as u8 - 0x40) as char);
                buf.as_str()
            }
            (Char(c), CTRL, ..) if c <= 0xff as char => {
                // If shift is not held we have C == 0x63 and want to translate
                // that into 0x03
                buf.push((c as u8 - 0x60) as char);
                buf.as_str()
            }
            (Char(c), _, ALT, ..) if c <= 0xff as char => {
                // TODO: add config option to select 8-bit vs. escape behavior?
                //buf.push((c as u8 | 0x80) as char);
                buf.push(0x1b as char);
                buf.push(c);
                buf.as_str()
            }
            (Char(c), ..) => {
                buf.push(c);
                buf.as_str()
            }
            (Insert, _, _, SHIFT, _) => {
                let clip = host.get_clipboard()?;
                if self.bracketed_paste {
                    use std::fmt::Write;
                    write!(buf, "\x1b[200~{}\x1b[201~", clip)?;
                } else {
                    buf = clip;
                }
                buf.as_str()
            }

            (Up, _, _, _, APPCURSOR) => "\x1bOA",
            (Down, _, _, _, APPCURSOR) => "\x1bOB",
            (Right, _, _, _, APPCURSOR) => "\x1bOC",
            (Left, _, _, _, APPCURSOR) => "\x1bOD",
            (Home, _, _, _, APPCURSOR) => "\x1bOH",
            (End, _, _, _, APPCURSOR) => "\x1bOF",

            (Up, ..) => "\x1b[A",
            (Down, ..) => "\x1b[B",
            (Right, ..) => "\x1b[C",
            (Left, ..) => "\x1b[D",
            (PageUp, _, _, SHIFT, _) => {
                let rows = self.screen().physical_rows as i64;
                self.scroll_viewport(-rows);
                ""
            }
            (PageDown, _, _, SHIFT, _) => {
                let rows = self.screen().physical_rows as i64;
                self.scroll_viewport(rows);
                ""
            }
            (PageUp, ..) => "\x1b[5~",
            (PageDown, ..) => "\x1b[6~",
            (Home, ..) => "\x1b[H",
            (End, ..) => "\x1b[F",
            (Insert, ..) => "\x1b[2~",

            // Modifier keys pressed on their own and unmappable keys don't expand to anything
            (Control, ..) | (Alt, ..) | (Meta, ..) | (Super, ..) | (Hyper, ..) | (Shift, ..) |
            (Unknown, ..) => "",
        };

        host.writer().write(&to_send.as_bytes())?;

        // Reset the viewport if we sent data to the parser
        if to_send.len() > 0 && self.viewport_offset != 0 {
            // TODO: some folks like to configure this behavior.
            self.set_scroll_viewport(0);
        }

        Ok(())
    }

    pub fn key_up(
        &mut self,
        _: KeyCode,
        _: KeyModifiers,
        _: &mut TerminalHost,
    ) -> Result<(), Error> {
        Ok(())
    }

    pub fn resize(&mut self, physical_rows: usize, physical_cols: usize) {
        self.screen.resize(physical_rows, physical_cols);
        self.alt_screen.resize(physical_rows, physical_cols);
        self.scroll_region = 0..physical_rows as i64;
        self.set_scroll_viewport(0);
    }

    /// Returns true if any of the visible lines are marked dirty
    pub fn has_dirty_lines(&self) -> bool {
        let screen = self.screen();
        let height = screen.physical_rows;
        let len = screen.lines.len() - self.viewport_offset as usize;

        for line in screen.lines.iter().skip(len - height) {
            if line.is_dirty() {
                return true;
            }
        }

        false
    }

    /// Returns the set of visible lines that are dirty.
    /// The return value is a Vec<(line_idx, line, selrange)>, where
    /// line_idx is relative to the top of the viewport.
    /// The selrange value is the column range representing the selected
    /// columns on this line.
    pub fn get_dirty_lines(&self) -> Vec<(usize, &Line, Range<usize>)> {
        let mut res = Vec::new();

        let screen = self.screen();
        let height = screen.physical_rows;
        let len = screen.lines.len() - self.viewport_offset as usize;

        let selection = self.selection_range.map(|r| r.normalize());

        for (i, mut line) in screen.lines.iter().skip(len - height).enumerate() {
            if i >= height {
                // When scrolling back, make sure we don't emit lines that
                // are below the bottom of the viewport
                break;
            }
            if line.is_dirty() {
                let selrange = match selection {
                    None => 0..0,
                    Some(sel) => {
                        // i is relative to the viewport, convert it back to
                        // something we can relate to the selection
                        let row = (i as ScrollbackOrVisibleRowIndex) -
                            self.viewport_offset as ScrollbackOrVisibleRowIndex;
                        sel.cols_for_row(row)
                    }
                };
                res.push((i, &*line, selrange));
            }
        }

        res
    }

    pub fn get_viewport_offset(&self) -> VisibleRowIndex {
        self.viewport_offset
    }

    /// Clear the dirty flag for all dirty lines
    pub fn clean_dirty_lines(&mut self) {
        let screen = self.screen_mut();
        for line in screen.lines.iter_mut() {
            line.set_clean();
        }
    }

    /// When dealing with selection, mark a range of lines as dirty
    pub fn make_all_lines_dirty(&mut self) {
        let screen = self.screen_mut();
        for line in screen.lines.iter_mut() {
            line.set_dirty();
        }
    }

    /// Returns the 0-based cursor position relative to the top left of
    /// the visible screen
    pub fn cursor_pos(&self) -> CursorPosition {
        // TODO: figure out how to expose cursor visibility; Option<CursorPosition>?
        CursorPosition {
            x: self.cursor.x,
            y: self.cursor.y + self.viewport_offset,
        }
    }

    /// Returns the currently highlighted hyperlink
    pub fn current_highlight(&self) -> Option<Rc<Hyperlink>> {
        self.current_highlight.as_ref().cloned()
    }

    /// Sets the cursor position. x and y are 0-based and relative to the
    /// top left of the visible screen.
    /// TODO: DEC origin mode impacts the interpreation of these
    fn set_cursor_pos(&mut self, x: &Position, y: &Position) {
        let x = match x {
            &Position::Relative(x) => (self.cursor.x as i64 + x).max(0),
            &Position::Absolute(x) => x,
        };
        let y = match y {
            &Position::Relative(y) => (self.cursor.y + y).max(0),
            &Position::Absolute(y) => y,
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

    fn set_scroll_viewport(&mut self, position: VisibleRowIndex) {
        self.clear_selection();
        let position = position.max(0);

        let rows = self.screen().physical_rows;
        let avail_scrollback = self.screen().lines.len() - rows;

        let position = position.min(avail_scrollback as i64);

        self.viewport_offset = position;
        let top = self.screen().lines.len() - (rows + position as usize);
        {
            let screen = self.screen_mut();
            for y in top..top + rows {
                screen.line_mut(y).set_dirty();
            }
        }
        self.recompute_highlight();
    }

    /// Adjust the scroll position of the viewport by delta.
    /// Dirties the lines that are now in view.
    pub fn scroll_viewport(&mut self, delta: VisibleRowIndex) {
        let position = self.viewport_offset - delta;
        self.set_scroll_viewport(position);
    }

    fn scroll_up(&mut self, num_rows: usize) {
        self.clear_selection();
        let scroll_region = self.scroll_region.clone();
        self.screen_mut().scroll_up(&scroll_region, num_rows)
    }

    fn scroll_down(&mut self, num_rows: usize) {
        self.clear_selection();
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

    fn push_answerback(&mut self, buf: &[u8]) {
        self.answerback.push(AnswerBack::WriteToPty(buf.to_vec()));
    }

    pub(crate) fn drain_answerback(&mut self) -> Option<Vec<AnswerBack>> {
        if self.answerback.len() == 0 {
            None
        } else {
            Some(self.answerback.drain(0..).collect())
        }
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
    /// the cursor moves to the right margin. HT does not cause text to auto wrap.
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
            Some(hyperlink) => Some(Rc::new(hyperlink)),
            None => None,
        }
    }

    fn perform_csi(&mut self, act: CSIAction) {
        debug!("{:?}", act);
        match act {
            CSIAction::DeleteCharacter(n) => {
                let y = self.cursor.y;
                let x = self.cursor.x;
                let screen = self.screen_mut();
                let limit = (x + n as usize).min(screen.physical_cols);
                for _ in x..limit as usize {
                    screen.erase_cell(x, y);
                }
            }
            CSIAction::EraseCharacter(n) => {
                let y = self.cursor.y;
                let x = self.cursor.x;
                let screen = self.screen_mut();
                let blank = CellAttributes::default();
                let limit = (x + n as usize).min(screen.physical_cols);
                for x in x..limit as usize {
                    screen.set_cell(x, y, ' ', &blank);
                }
            }
            CSIAction::SoftReset => {
                self.pen = CellAttributes::default();
                // TODO: see https://vt100.net/docs/vt510-rm/DECSTR.html
            }
            CSIAction::SetPenNoLink(pen) => {
                let link = self.pen.hyperlink.take();
                self.pen = pen;
                self.pen.hyperlink = link;
            }
            CSIAction::SetForegroundColor(color) => {
                self.pen.foreground = color;
            }
            CSIAction::SetBackgroundColor(color) => {
                self.pen.background = color;
            }
            CSIAction::SetIntensity(level) => {
                self.pen.set_intensity(level);
            }
            CSIAction::SetUnderline(level) => {
                self.pen.set_underline(level);
            }
            CSIAction::SetItalic(on) => {
                self.pen.set_italic(on);
            }
            CSIAction::SetBlink(on) => {
                self.pen.set_blink(on);
            }
            CSIAction::SetReverse(on) => {
                self.pen.set_reverse(on);
            }
            CSIAction::SetStrikethrough(on) => {
                self.pen.set_strikethrough(on);
            }
            CSIAction::SetInvisible(on) => {
                self.pen.set_invisible(on);
            }
            CSIAction::SetCursorXY { x, y } => {
                self.set_cursor_pos(&x, &y);
            }
            CSIAction::EraseInLine(erase) => {
                let cx = self.cursor.x;
                let cy = self.cursor.y;
                let mut screen = self.screen_mut();
                let cols = screen.physical_cols;
                match erase {
                    LineErase::ToRight => {
                        screen.clear_line(cy, cx..cols);
                    }
                    LineErase::ToLeft => {
                        screen.clear_line(cy, 0..cx);
                    }
                    LineErase::All => {
                        screen.clear_line(cy, 0..cols);
                    }
                }
            }
            CSIAction::EraseInDisplay(erase) => {
                let cy = self.cursor.y;
                let mut screen = self.screen_mut();
                let cols = screen.physical_cols;
                let rows = screen.physical_rows as VisibleRowIndex;
                match erase {
                    DisplayErase::Below => {
                        for y in cy..rows {
                            screen.clear_line(y, 0..cols);
                        }
                    }
                    DisplayErase::Above => {
                        for y in 0..cy {
                            screen.clear_line(y, 0..cols);
                        }
                    }
                    DisplayErase::All => {
                        for y in 0..rows {
                            screen.clear_line(y, 0..cols);
                        }
                    }
                    DisplayErase::SavedLines => {
                        println!("ed: no support for xterm Erase Saved Lines yet");
                    }
                }
            }
            CSIAction::SetDecPrivateMode(DecPrivateMode::StartBlinkingCursor, _) => {
                // ignored
            }
            CSIAction::SetDecPrivateMode(DecPrivateMode::ShowCursor, on) => {
                self.cursor_visible = on;
            }
            CSIAction::SetDecPrivateMode(DecPrivateMode::ButtonEventMouse, on) => {
                self.button_event_mouse = on;
            }
            CSIAction::SetDecPrivateMode(DecPrivateMode::SGRMouse, on) => {
                self.sgr_mouse = on;
            }
            CSIAction::SetDecPrivateMode(DecPrivateMode::ClearAndEnableAlternateScreen, on) => {
                // TODO: some folks like to disable alt screen
                match (on, self.alt_screen_is_active) {
                    (true, false) => {
                        self.perform_csi(CSIAction::SaveCursor);
                        self.alt_screen_is_active = true;
                        self.set_cursor_pos(&Position::Absolute(0), &Position::Absolute(0));
                        self.perform_csi(CSIAction::EraseInDisplay(DisplayErase::All));
                        self.set_scroll_viewport(0);
                    }
                    (false, true) => {
                        self.alt_screen_is_active = false;
                        self.perform_csi(CSIAction::RestoreCursor);
                        self.set_scroll_viewport(0);
                    }
                    _ => {}
                }
            }
            CSIAction::SetDecPrivateMode(DecPrivateMode::ApplicationCursorKeys, on) => {
                self.application_cursor_keys = on;
            }
            CSIAction::SetDecPrivateMode(DecPrivateMode::BrackedPaste, on) => {
                self.bracketed_paste = on;
            }
            CSIAction::DeviceStatusReport => {
                // "OK"
                self.push_answerback(b"\x1b[0n");
            }
            CSIAction::ReportCursorPosition => {
                let row = self.cursor.y + 1;
                let col = self.cursor.x + 1;
                self.push_answerback(format!("\x1b[{};{}R", row, col).as_bytes());
            }
            CSIAction::SetScrollingRegion { top, bottom } => {
                let rows = self.screen().physical_rows;
                let mut top = top.min(rows as i64 - 1);
                let mut bottom = bottom.min(rows as i64 - 1);
                if top > bottom {
                    std::mem::swap(&mut top, &mut bottom);
                }
                self.scroll_region = top..bottom + 1;
            }
            CSIAction::RequestDeviceAttributes => {
                self.push_answerback(DEVICE_IDENT);
            }
            CSIAction::DeleteLines(n) => {
                if in_range(self.cursor.y, &self.scroll_region) {
                    let scroll_region = self.cursor.y..self.scroll_region.end;
                    self.screen_mut().scroll_up(&scroll_region, n as usize);
                }
            }
            CSIAction::InsertLines(n) => {
                if in_range(self.cursor.y, &self.scroll_region) {
                    let scroll_region = self.cursor.y..self.scroll_region.end;
                    self.screen_mut().scroll_down(&scroll_region, n as usize);
                }
            }
            CSIAction::SaveCursor => {
                self.saved_cursor = self.cursor;
            }
            CSIAction::RestoreCursor => {
                let x = self.saved_cursor.x;
                let y = self.saved_cursor.y;
                self.set_cursor_pos(&Position::Absolute(x as i64), &Position::Absolute(y));
            }
            CSIAction::LinePosition(row) => {
                self.set_cursor_pos(&Position::Relative(0), &row);
            }
            CSIAction::ScrollLines(amount) => {
                if amount > 0 {
                    self.scroll_down(amount as usize);
                } else {
                    self.scroll_up((-amount) as usize);
                }
            }
        }
    }
}

impl vte::Perform for TerminalState {
    /// Draw a character to the screen
    fn print(&mut self, c: char) {
        if self.wrap_next {
            self.new_line(true);
        }

        let x = self.cursor.x;
        let y = self.cursor.y;
        let width = self.screen().physical_cols;

        let pen = self.pen.clone();

        // Assign the cell and extract its printable width
        let print_width = {
            let cell = self.screen_mut().set_cell(x, y, c, &pen);
            cell.width()
        };

        // for double- or triple-wide cells, the client of the terminal
        // expects the cursor to move by the visible width, which means that
        // we need to generate non-printing cells to pad out the gap.  They
        // need to be non-printing rather than space so that that renderer
        // doesn't render an actual space between the glyphs.
        for non_print_x in 1..print_width {
            self.screen_mut().set_cell(
                x + non_print_x,
                y,
                0 as char, // non-printable
                &pen,
            );
        }

        if x + print_width < width {
            self.cursor.x += print_width;
            self.wrap_next = false;
        } else {
            self.wrap_next = true;
        }
    }

    fn execute(&mut self, byte: u8) {
        debug!("execute {:02x}", byte);
        match byte {
            b'\n' | 0x0b /* VT */ | 0x0c /* FF */ => {
                self.new_line(true /* TODO: depend on terminal mode */)
            }
            b'\r' => /* CR */ {
                self.set_cursor_pos(&Position::Absolute(0), &Position::Relative(0));
            }
            0x08 /* BS */ => {
                self.set_cursor_pos(&Position::Relative(-1), &Position::Relative(0));
            }
            b'\t' => self.c0_horizontal_tab(),
            _ => println!("unhandled vte execute {}", byte),
        }
    }
    fn hook(&mut self, _: &[i64], _: &[u8], _: bool) {}
    fn put(&mut self, _: u8) {}
    fn unhook(&mut self) {}
    fn osc_dispatch(&mut self, osc: &[&[u8]]) {
        match osc {
            &[b"0", title] => {
                if let Ok(title) = str::from_utf8(title) {
                    self.answerback.push(
                        AnswerBack::TitleChanged(title.to_string()),
                    );
                } else {
                    eprintln!("OSC: failed to decode utf title for {:?}", title);
                }
            }
            &[b"8", params, url] => {
                // Hyperlinks per:
                // https://gist.github.com/egmontkob/eb114294efbcd5adb1944c9f3cb5feda
                match (str::from_utf8(params), str::from_utf8(url)) {
                    (Ok(params), Ok(url)) => {
                        let params = hyperlink::parse_link_params(params);
                        if url.len() > 0 {
                            self.set_hyperlink(Some(Hyperlink::new(url, &params)));
                        } else {
                            self.set_hyperlink(None);
                        }
                    }
                    _ => {
                        eprintln!("problem decoding URL/params {:?}, {:?}", url, params);
                        self.set_hyperlink(None)
                    }
                }
            }
            _ => {
                if osc.len() > 0 {
                    eprintln!("OSC unhandled: {:?} {:?}", str::from_utf8(osc[0]), osc);
                } else {
                    eprintln!("OSC unhandled: {:?}", osc);
                }
            }
        }
    }
    fn csi_dispatch(&mut self, params: &[i64], intermediates: &[u8], ignore: bool, byte: char) {
        /*
        println!(
            "CSI params={:?}, intermediates={:?} b={:02x} {}",
            params,
            intermediates,
            byte as u8,
            byte ,
        );
        */
        for act in CSIParser::new(params, intermediates, ignore, byte) {
            self.perform_csi(act);
        }
    }

    fn esc_dispatch(&mut self, params: &[i64], intermediates: &[u8], _ignore: bool, byte: u8) {
        debug!(
            "ESC params={:?}, intermediates={:?} b={:02x} {}",
            params,
            intermediates,
            byte,
            byte as char
        );
        // Sequences from both of these sections show up in this handler:
        // https://invisible-island.net/xterm/ctlseqs/ctlseqs.html#h2-C1-_8-Bit_-Control-Characters
        // https://invisible-island.net/xterm/ctlseqs/ctlseqs.html#h2-Controls-beginning-with-ESC
        match (byte, intermediates, params) {
            // String Terminator (ST); explicitly has nothing to do here, as its purpose is
            // handled by vte::Parser
            (b'\\', &[], &[]) => {}
            // Application Keypad (DECKPAM)
            (b'=', &[], &[]) => {
                debug!("DECKPAM on");
                self.application_keypad = true;
            }
            // Normal Keypad (DECKPAM)
            (b'>', &[], &[]) => {
                debug!("DECKPAM off");
                self.application_keypad = false;
            }
            // Reverse Index (RI)
            (b'M', &[], &[]) => self.c1_reverse_index(),
            // Index (IND)
            (b'D', &[], &[]) => self.c1_index(),
            // Next Line (NEL)
            (b'E', &[], &[]) => self.c1_nel(),
            // Horizontal Tab Set (HTS)
            (b'H', &[], &[]) => self.c1_hts(),

            // Enable alternate character set mode (smacs)
            (b'0', &[b'('], &[]) => {
                debug!("ESC: smacs");
            }
            // Exit alternate character set mode (rmacs)
            (b'B', &[b'('], &[]) => {
                debug!("ESC: rmacs");
            }

            // DECSC - Save Cursor
            (b'7', &[], &[]) => self.perform_csi(CSIAction::SaveCursor),
            // DECRC - Restore Cursor
            (b'8', &[], &[]) => self.perform_csi(CSIAction::RestoreCursor),

            (..) => {
                println!(
                    "ESC unhandled params={:?}, intermediates={:?} b={:02x} {}",
                    params,
                    intermediates,
                    byte,
                    byte as char
                );
            }
        }
    }
}
