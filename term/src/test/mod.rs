//! Various tests of the terminal model and escape sequence
//! processing routines.

use super::*;
mod c0;
use bitflags::bitflags;
mod c1;
mod csi;
use failure::Fallible;
mod selection;
use pretty_assertions::assert_eq;
use std::cell::RefCell;
use std::sync::Arc;
use termwiz::escape::csi::{Edit, EraseInDisplay, EraseInLine};
use termwiz::escape::{OneBased, OperatingSystemCommand, CSI};

struct TestHost {
    title: String,
    clip: Arc<dyn Clipboard>,
}

impl TestHost {
    fn new() -> Self {
        Self {
            title: String::new(),
            clip: Arc::new(LocalClip::new()),
        }
    }
}

impl std::io::Write for TestHost {
    fn write(&mut self, _buf: &[u8]) -> Result<usize, std::io::Error> {
        panic!("no writer support in TestHost");
    }
    fn flush(&mut self) -> Result<(), std::io::Error> {
        panic!("no writer support in TestHost");
    }
}

#[derive(Debug)]
struct LocalClip {
    clip: RefCell<Option<String>>,
}

impl LocalClip {
    fn new() -> Self {
        Self {
            clip: RefCell::new(None),
        }
    }
}

impl Clipboard for LocalClip {
    fn set_contents(&self, clip: Option<String>) -> Fallible<()> {
        *self.clip.borrow_mut() = clip;
        Ok(())
    }

    fn get_contents(&self) -> Fallible<String> {
        self.clip
            .borrow()
            .as_ref()
            .map(|c| c.clone())
            .ok_or_else(|| failure::err_msg("no clipboard"))
    }
}

impl TerminalHost for TestHost {
    fn set_title(&mut self, title: &str) {
        self.title = title.into();
    }

    fn get_clipboard(&mut self) -> Fallible<Arc<dyn Clipboard>> {
        Ok(Arc::clone(&self.clip))
    }

    fn writer(&mut self) -> &mut dyn std::io::Write {
        self
    }

    fn click_link(&mut self, _link: &Arc<Hyperlink>) {}
}

struct TestTerm {
    term: Terminal,
    host: TestHost,
}

impl TestTerm {
    fn new(height: usize, width: usize, scrollback: usize) -> Self {
        Self {
            term: Terminal::new(
                height,
                width,
                height * 16,
                width * 8,
                scrollback,
                Vec::new(),
            ),
            host: TestHost::new(),
        }
    }

    fn print<B: AsRef<[u8]>>(&mut self, bytes: B) {
        self.term.advance_bytes(bytes, &mut self.host);
    }

    #[allow(dead_code)]
    fn set_mode(&mut self, mode: &str, enable: bool) {
        self.print(CSI);
        self.print(mode);
        self.print(if enable { b"h" } else { b"l" });
    }

    #[allow(dead_code)]
    fn set_scroll_region(&mut self, top: usize, bottom: usize) {
        self.print(CSI);
        self.print(format!("{};{}r", top + 1, bottom + 1));
    }

    fn delete_lines(&mut self, n: isize) {
        self.print(CSI);
        self.print(format!("{}M", n));
    }

    fn cup(&mut self, col: isize, row: isize) {
        self.print(CSI);
        self.print(format!("{};{}H", row + 1, col + 1));
    }

    fn hvp(&mut self, col: isize, row: isize) {
        self.print(CSI);
        self.print(format!("{};{}f", row + 1, col + 1));
    }

    fn erase_in_display(&mut self, erase: EraseInDisplay) {
        let csi = CSI::Edit(Edit::EraseInDisplay(erase));
        self.print(format!("{}", csi));
    }

    fn erase_in_line(&mut self, erase: EraseInLine) {
        let csi = CSI::Edit(Edit::EraseInLine(erase));
        self.print(format!("{}", csi));
    }

    fn hyperlink(&mut self, link: &Arc<Hyperlink>) {
        let osc = OperatingSystemCommand::SetHyperlink(Some(link.as_ref().clone()));
        self.print(format!("{}", osc));
    }

    fn hyperlink_off(&mut self) {
        self.print("\x1b]8;;\x1b\\");
    }

    fn soft_reset(&mut self) {
        self.print(CSI);
        self.print("!p");
    }

    fn mouse(&mut self, event: MouseEvent) -> Result<(), Error> {
        self.term.mouse_event(event, &mut self.host)
    }

    fn get_clipboard(&self) -> Option<String> {
        self.host.clip.get_contents().ok()
    }

    /// Inject n_times clicks of the button at the specified coordinates
    fn click_n(&mut self, x: usize, y: i64, button: MouseButton, n_times: usize) {
        for _ in 0..n_times {
            self.mouse(MouseEvent {
                kind: MouseEventKind::Press,
                x,
                y,
                button,
                modifiers: KeyModifiers::default(),
            })
            .unwrap();
            self.mouse(MouseEvent {
                kind: MouseEventKind::Release,
                x,
                y,
                button,
                modifiers: KeyModifiers::default(),
            })
            .unwrap();
        }
    }

    /// Left mouse button drag from the start to the end coordinates
    fn drag_select(&mut self, start_x: usize, start_y: i64, end_x: usize, end_y: i64) {
        // Break any outstanding click streak that might falsely trigger due to
        // this unit test happening much faster than the CLICK_INTERVAL allows.
        self.click_n(0, 0, MouseButton::Right, 1);

        // Now inject the appropriate left click events

        self.mouse(MouseEvent {
            kind: MouseEventKind::Press,
            x: start_x,
            y: start_y,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        })
        .unwrap();
        assert!(self.get_clipboard().is_none());

        self.mouse(MouseEvent {
            kind: MouseEventKind::Move,
            x: end_x,
            y: end_y,
            button: MouseButton::None,
            modifiers: KeyModifiers::default(),
        })
        .unwrap();
        assert!(self.get_clipboard().is_none());

        self.mouse(MouseEvent {
            kind: MouseEventKind::Release,
            x: end_x,
            y: end_y,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        })
        .unwrap();
    }

    fn assert_cursor_pos(&self, x: usize, y: i64, reason: Option<&str>) {
        let cursor = self.cursor_pos();
        let expect = CursorPosition { x, y };
        assert_eq!(
            cursor, expect,
            "actual cursor (left) didn't match expected cursor (right) reason={:?}",
            reason
        );
    }

    fn assert_dirty_lines(&self, expected: &[usize], reason: Option<&str>) {
        let dirty_indices: Vec<usize> = self.get_dirty_lines().iter().map(|&(i, ..)| i).collect();
        assert_eq!(
            &dirty_indices, &expected,
            "actual dirty lines (left) didn't match expected dirty lines (right) reason={:?}",
            reason
        );
    }

    fn viewport_lines(&self) -> Vec<Line> {
        let screen = self.screen();
        let line_count = screen.lines.len();
        let viewport = self.viewport_offset;
        let phs_rows = screen.physical_rows;
        screen
            .all_lines()
            .iter()
            .skip((line_count as i64 - phs_rows as i64 - viewport) as usize)
            .take(phs_rows)
            .cloned()
            .collect()
    }
    fn print_viewport_lines(&self) {
        println!("viewport contents are:");
        for line in self.viewport_lines() {
            println!("[{}]", line.as_str());
        }
    }

    fn assert_viewport_contents(&self, expect_lines: &[&str]) {
        self.print_viewport_lines();

        let expect: Vec<Line> = expect_lines.iter().map(|s| (*s).into()).collect();

        assert_lines_equal(&self.viewport_lines(), &expect, Compare::TEXT);
    }
}

impl Deref for TestTerm {
    type Target = Terminal;

    fn deref(&self) -> &Terminal {
        &self.term
    }
}

impl DerefMut for TestTerm {
    fn deref_mut(&mut self) -> &mut Terminal {
        &mut self.term
    }
}

/// Asserts that both line slices match according to the
/// selected flags.
fn assert_lines_equal(lines: &[Line], expect_lines: &[Line], compare: Compare) {
    let mut expect_iter = expect_lines.iter();

    for (idx, line) in lines.iter().enumerate() {
        let expect = expect_iter.next().unwrap();

        if compare.contains(Compare::DIRTY) {
            assert_eq!(
                line.is_dirty(),
                expect.is_dirty(),
                "line {} dirty didn't match",
                idx,
            );
        }

        if compare.contains(Compare::ATTRS) {
            let line_attrs: Vec<_> = line.cells().iter().map(|c| c.attrs().clone()).collect();
            let expect_attrs: Vec<_> = expect.cells().iter().map(|c| c.attrs().clone()).collect();
            assert_eq!(
                expect_attrs,
                line_attrs,
                "line {} `{}` attrs didn't match (left=expected, right=actual)",
                idx,
                line.as_str()
            );
        }
        if compare.contains(Compare::TEXT) {
            let line_str = line.as_str();
            let expect_str = expect.as_str();
            assert_eq!(line_str, expect_str, "line {} text didn't match", idx,);
        }
    }

    assert_eq!(
        lines.len(),
        expect_lines.len(),
        "expectation has wrong number of lines"
    );
}

bitflags! {
    struct Compare : u8{
        const TEXT = 1;
        const ATTRS = 2;
        const DIRTY = 4;
    }
}

fn print_all_lines(term: &Terminal) {
    let screen = term.screen();

    println!("whole screen contents are:");
    for line in screen.lines.iter() {
        println!("[{}]", line.as_str());
    }
}

fn print_visible_lines(term: &Terminal) {
    let screen = term.screen();

    println!("screen contents are:");
    for line in screen.visible_lines().iter() {
        println!("[{}]", line.as_str());
    }
}

/// Asserts that the visible lines of the terminal have the
/// same character contents as the expected lines.
/// The other cell attributes are not compared; this is
/// a convenience for writing visually understandable tests.
fn assert_visible_contents(term: &Terminal, expect_lines: &[&str]) {
    print_visible_lines(&term);
    let screen = term.screen();

    let expect: Vec<Line> = expect_lines.iter().map(|s| (*s).into()).collect();

    assert_lines_equal(&screen.visible_lines(), &expect, Compare::TEXT);
}

fn assert_all_contents(term: &Terminal, expect_lines: &[&str]) {
    print_all_lines(&term);
    let screen = term.screen();

    let expect: Vec<Line> = expect_lines.iter().map(|s| (*s).into()).collect();

    assert_lines_equal(&screen.all_lines(), &expect, Compare::TEXT);
}

#[test]
fn basic_output() {
    let mut term = TestTerm::new(5, 10, 0);

    term.cup(1, 1);
    term.print("hello, world!");
    assert_visible_contents(
        &term,
        &[
            "          ",
            " hello, wo",
            "rld!      ",
            "          ",
            "          ",
        ],
    );

    term.erase_in_display(EraseInDisplay::EraseToStartOfDisplay);
    assert_visible_contents(
        &term,
        &[
            "          ",
            "          ",
            "          ",
            "          ",
            "          ",
        ],
    );

    term.cup(0, 2);
    term.print("woot");
    term.cup(2, 2);
    term.erase_in_line(EraseInLine::EraseToEndOfLine);
    assert_visible_contents(
        &term,
        &[
            "          ",
            "          ",
            "wo        ",
            "          ",
            "          ",
        ],
    );

    term.erase_in_line(EraseInLine::EraseToStartOfLine);
    assert_visible_contents(
        &term,
        &[
            "          ",
            "          ",
            "          ",
            "          ",
            "          ",
        ],
    );
}

/// Ensure that we dirty lines as the cursor is moved around, otherwise
/// the renderer won't draw the cursor in the right place
#[test]
fn cursor_movement_damage() {
    let mut term = TestTerm::new(2, 3, 0);

    term.print("fooo.");
    assert_visible_contents(&term, &["foo", "o. "]);
    term.assert_cursor_pos(2, 1, None);
    term.assert_dirty_lines(&[0, 1], None);

    term.cup(0, 1);
    term.clean_dirty_lines();
    term.print("\x08");
    term.assert_cursor_pos(0, 1, Some("BS doesn't change the line"));
    term.assert_dirty_lines(&[1], None);
    term.clean_dirty_lines();

    term.cup(0, 0);
    term.assert_dirty_lines(&[0, 1], Some("cursor movement dirties old and new lines"));
}

/// Replicates a bug I initially found via:
/// $ vim
/// :help
/// PageDown
#[test]
fn test_delete_lines() {
    let mut term = TestTerm::new(5, 3, 0);

    term.print("111\r\n222\r\n333\r\n444\r\n555");
    assert_visible_contents(&term, &["111", "222", "333", "444", "555"]);
    term.assert_dirty_lines(&[0, 1, 2, 3, 4], None);
    term.cup(0, 1);
    term.clean_dirty_lines();

    term.assert_dirty_lines(&[], None);
    term.delete_lines(2);
    assert_visible_contents(&term, &["111", "444", "555", "   ", "   "]);
    term.assert_dirty_lines(&[1, 2, 3, 4], None);
    term.clean_dirty_lines();

    term.cup(0, 3);
    term.print("aaa\r\nbbb");
    term.cup(0, 1);
    term.clean_dirty_lines();
    assert_visible_contents(&term, &["111", "444", "555", "aaa", "bbb"]);

    // test with a scroll region smaller than the screen
    term.set_scroll_region(1, 3);
    print_all_lines(&term);
    term.delete_lines(2);

    assert_visible_contents(&term, &["111", "aaa", "   ", "   ", "bbb"]);
    term.assert_dirty_lines(&[1, 2, 3], None);

    // expand the scroll region to fill the screen
    term.set_scroll_region(0, 4);
    term.clean_dirty_lines();
    term.delete_lines(1);

    assert_visible_contents(&term, &["111", "   ", "   ", "bbb", "   "]);
    term.assert_dirty_lines(&[1, 2, 3, 4], None);
}

#[test]
fn test_scrollup() {
    let mut term = TestTerm::new(2, 1, 4);
    term.print("1\n");
    assert_all_contents(&term, &["1", " "]);
    term.print("2\n");
    assert_all_contents(&term, &["1", "2", " "]);
    term.print("3\n");
    assert_all_contents(&term, &["1", "2", "3", " "]);
    term.print("4\n");
    assert_all_contents(&term, &["1", "2", "3", "4", " "]);
    term.print("5\n");
    assert_all_contents(&term, &["1", "2", "3", "4", "5", " "]);
    term.print("6\n");
    assert_all_contents(&term, &["2", "3", "4", "5", "6", " "]);
    term.print("7\n");
    assert_all_contents(&term, &["3", "4", "5", "6", "7", " "]);
    term.print("8\n");
    assert_all_contents(&term, &["4", "5", "6", "7", "8", " "]);
}

#[test]
fn test_scroll_margins() {
    let mut term = TestTerm::new(3, 1, 10);
    term.print("1\n2\n3\n4\n");
    assert_all_contents(&term, &["1", "2", "3", "4", " "]);

    let margins = CSI::Cursor(termwiz::escape::csi::Cursor::SetTopAndBottomMargins {
        top: OneBased::new(1),
        bottom: OneBased::new(2),
    });
    term.print(format!("{}", margins));

    term.print("z\n");
    assert_all_contents(&term, &["1", "2", "3", "4", "z"]);

    term.print("a\n");
    assert_all_contents(&term, &["1", "2", "3", "4", "a"]);

    term.cup(0, 1);
    term.print("W\n");
    assert_all_contents(&term, &["1", "2", "3", "W", " ", "a"]);
}

#[test]
fn test_hyperlinks() {
    let mut term = TestTerm::new(3, 5, 0);
    let link = Arc::new(Hyperlink::new("http://example.com"));
    term.hyperlink(&link);
    term.print("hello");
    term.hyperlink_off();

    let mut linked = CellAttributes::default();
    linked.hyperlink = Some(Arc::clone(&link));

    assert_lines_equal(
        &term.screen().visible_lines(),
        &[
            Line::from_text_with_wrapped_last_col("hello", &linked),
            Line::from_text("     ", &CellAttributes::default()),
            Line::from_text("     ", &CellAttributes::default()),
        ],
        Compare::TEXT | Compare::ATTRS,
    );

    term.hyperlink(&link);
    term.print("he");
    // Resetting pen should not reset the link
    term.print("\x1b[m");
    term.print("y!!");

    assert_lines_equal(
        &term.screen().visible_lines(),
        &[
            Line::from_text_with_wrapped_last_col("hello", &linked),
            Line::from_text_with_wrapped_last_col("hey!!", &linked),
            "     ".into(),
        ],
        Compare::TEXT | Compare::ATTRS,
    );

    let otherlink = Arc::new(Hyperlink::new_with_id("http://example.com/other", "w00t"));

    // Switching link and turning it off
    term.hyperlink(&otherlink);
    term.print("wo");
    // soft reset also disables hyperlink attribute
    term.soft_reset();
    term.print("00t");

    let mut partial_line =
        Line::from_text_with_wrapped_last_col("wo00t", &CellAttributes::default());
    partial_line.set_cell(
        0,
        Cell::new(
            'w',
            CellAttributes::default()
                .set_hyperlink(Some(Arc::clone(&otherlink)))
                .clone(),
        ),
    );
    partial_line.set_cell(
        1,
        Cell::new(
            'o',
            CellAttributes::default()
                .set_hyperlink(Some(Arc::clone(&otherlink)))
                .clone(),
        ),
    );

    assert_lines_equal(
        &term.screen().visible_lines(),
        &[
            Line::from_text_with_wrapped_last_col("hello", &linked),
            Line::from_text_with_wrapped_last_col("hey!!", &linked),
            partial_line,
        ],
        Compare::TEXT | Compare::ATTRS,
    );
}
