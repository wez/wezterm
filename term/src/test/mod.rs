//! Various tests of the terminal model and escape sequence
//! processing routines.

use super::*;

#[derive(Default, Debug)]
struct TestHost {
    title: String,
    clip: Option<String>,
}

impl TestHost {
    fn new() -> Self {
        Self::default()
    }
}
impl TerminalHost for TestHost {
    fn set_title(&mut self, title: &str) {
        self.title = title.into();
    }

    fn set_clipboard(&mut self, clip: Option<String>) -> Result<(), Error> {
        self.clip = clip;
        Ok(())
    }

    fn get_clipboard(&mut self) -> Result<String, Error> {
        self.clip.as_ref().map(|c| c.clone()).ok_or_else(|| {
            failure::err_msg("no clipboard")
        })
    }

    fn writer(&mut self) -> &mut std::io::Write {
        panic!("no writer support in TestHost");
    }

    fn click_link(&mut self, _link: &Rc<Hyperlink>) {}
}

macro_rules! assert_cursor_pos {
    ($term:expr, $x:expr, $y:expr) => {
        assert_cursor_pos!($term, $x, $y,
            "actual cursor (left) didn't match expected cursor (right)");
    };

    ($term:expr, $x:expr, $y:expr, $($reason:tt)*) => {
        {
            let cursor = $term.cursor_pos();
            let expect = CursorPosition { x: $x, y: $y };
            assert_eq!(cursor, expect, $($reason)*);
        }
    };
}

macro_rules! assert_dirty_lines {
    ($term:expr, $expected:expr) => {
        assert_dirty_lines!($term, $expected,
            "actual dirty lines (left) didn't match expected dirty lines (right)");
    };

    ($term:expr, $expected:expr, $($reason:tt)*) => {
        let dirty_indices: Vec<usize> = $term.get_dirty_lines()
                                             .iter()
                                             .map(|&(i, _, _)| i).collect();
        assert_eq!(&dirty_indices, $expected, $($reason)*);
    };
}

/// Asserts that both line slices match according to the
/// selected flags.
fn assert_lines_equal(lines: &[Line], expect_lines: &[Line], compare: Compare) {
    let mut expect_iter = expect_lines.iter();

    for (idx, line) in lines.iter().enumerate() {
        let expect = expect_iter.next().unwrap();

        if compare.contains(Compare::DIRTY) {
            assert_eq!(
                line.is_dirty() , expect.is_dirty(),
                "line {} dirty didn't match",
                idx,
            );
        }

        if compare.contains(Compare::ATTRS) {
            let line_attrs: Vec<_> = line.cells.iter().map(|c| c.attrs.clone()).collect();
            let expect_attrs: Vec<_> = expect.cells.iter().map(|c| c.attrs.clone()).collect();
            assert_eq!(
                expect_attrs ,line_attrs,
                "line {} attrs didn't match",
                idx,
            );
        }
        if compare.contains(Compare::TEXT) {
            let line_str = line.as_str();
            let expect_str = expect.as_str();
            assert_eq!(
                line_str ,expect_str,
                "line {} text didn't match",
                idx,
            );
        }
    }

    assert_eq!(
        lines.len(),
        expect_lines.len(),
        "expectation has wrong number of lines"
    );
}

#[allow(dead_code)]
fn set_mode(term: &mut Terminal, host: &mut TerminalHost, mode: &str, enable: bool) {
    term.advance_bytes(CSI, host);
    term.advance_bytes(mode, host);
    term.advance_bytes(if enable { b"h" } else { b"l" }, host);
}

#[allow(dead_code)]
fn set_scroll_region(term: &mut Terminal, host: &mut TerminalHost, top: usize, bottom: usize) {
    term.advance_bytes(CSI, host);
    term.advance_bytes(format!("{};{}r", top + 1, bottom + 1), host);
}

fn delete_lines(term: &mut Terminal, host: &mut TerminalHost, n: usize) {
    term.advance_bytes(CSI, host);
    term.advance_bytes(format!("{}M", n), host);
}

fn cup(term: &mut Terminal, host: &mut TerminalHost, col: isize, row: isize) {
    term.advance_bytes(CSI, host);
    term.advance_bytes(format!("{};{}H", row + 1, col + 1), host);
}

fn erase_in_display(term: &mut Terminal, host: &mut TerminalHost, erase: DisplayErase) {
    term.advance_bytes(CSI, host);
    let num = match erase {
        DisplayErase::Below => 0,
        DisplayErase::Above => 1,
        DisplayErase::All => 2,
        DisplayErase::SavedLines => 3,
    };
    term.advance_bytes(format!("{}J", num), host);
}

fn erase_in_line(term: &mut Terminal, host: &mut TerminalHost, erase: LineErase) {
    term.advance_bytes(CSI, host);
    let num = match erase {
        LineErase::ToRight => 0,
        LineErase::ToLeft => 1,
        LineErase::All => 2,
    };
    term.advance_bytes(format!("{}K", num), host);
}

bitflags! {
    struct Compare : u8{
        const TEXT = 1;
        const ATTRS = 2;
        const DIRTY = 3;
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
/// same cell contents.  The cells must exactly match.
#[allow(dead_code)]
fn assert_visible_lines(term: &Terminal, expect_lines: &[Line]) {
    print_visible_lines(&term);
    let screen = term.screen();

    assert_lines_equal(
        screen.visible_lines(),
        expect_lines,
        Compare::ATTRS | Compare::TEXT,
    );
}

/// Asserts that the visible lines of the terminal have the
/// same character contents as the expected lines.
/// The other cell attributes are not compared; this is
/// a convenience for writing visually understandable tests.
fn assert_visible_contents(term: &Terminal, expect_lines: &[&str]) {
    print_visible_lines(&term);
    let screen = term.screen();

    let expect: Vec<Line> = expect_lines.iter().map(|s| (*s).into()).collect();

    assert_lines_equal(screen.visible_lines(), &expect, Compare::TEXT);
}

#[test]
fn basic_output() {
    let mut term = Terminal::new(5, 10, 0);
    let mut host = TestHost::new();

    cup(&mut term, &mut host, 1, 1);
    term.advance_bytes("hello, world!", &mut host);
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

    erase_in_display(&mut term, &mut host, DisplayErase::Above);
    assert_visible_contents(
        &term,
        &[
            "          ",
            "          ",
            "rld!      ",
            "          ",
            "          ",
        ],
    );

    cup(&mut term, &mut host, 2, 2);
    erase_in_line(&mut term, &mut host, LineErase::ToRight);
    assert_visible_contents(
        &term,
        &[
            "          ",
            "          ",
            "rl        ",
            "          ",
            "          ",
        ],
    );

    erase_in_line(&mut term, &mut host, LineErase::ToLeft);
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
    let mut term = Terminal::new(2, 3, 0);
    let mut host = TestHost::new();

    term.advance_bytes("fooo.", &mut host);
    assert_visible_contents(&term, &["foo", "o. "]);
    assert_cursor_pos!(&term, 2, 1);
    assert_dirty_lines!(&term, &[0, 1]);

    cup(&mut term, &mut host, 0, 1);
    term.clean_dirty_lines();
    term.advance_bytes("\x08", &mut host);
    assert_cursor_pos!(&term, 0, 1, "BS doesn't change the line");
    assert_dirty_lines!(&term, &[1]);
    term.clean_dirty_lines();

    cup(&mut term, &mut host, 0, 0);
    assert_dirty_lines!(&term, &[0, 1], "cursor movement dirties old and new lines");
}

/// Replicates a bug I initially found via:
/// $ vim
/// :help
/// PageDown
#[test]
fn test_delete_lines() {
    let mut term = Terminal::new(5, 3, 0);
    let mut host = TestHost::new();

    term.advance_bytes("111\r\n222\r\n333\r\n444\r\n555", &mut host);
    assert_visible_contents(&term, &["111", "222", "333", "444", "555"]);
    assert_dirty_lines!(&term, &[0, 1, 2, 3, 4]);
    cup(&mut term, &mut host, 0, 1);
    term.clean_dirty_lines();

    assert_dirty_lines!(&term, &[]);
    delete_lines(&mut term, &mut host, 2);
    assert_visible_contents(&term, &["111", "444", "555", "   ", "   "]);
    assert_dirty_lines!(&term, &[1, 2, 3, 4]);
    term.clean_dirty_lines();

    cup(&mut term, &mut host, 0, 3);
    term.advance_bytes("aaa\r\nbbb", &mut host);
    cup(&mut term, &mut host, 0, 1);
    term.clean_dirty_lines();
    assert_visible_contents(&term, &["111", "444", "555", "aaa", "bbb"]);

    // test with a scroll region smaller than the screen
    set_scroll_region(&mut term, &mut host, 1, 3);
    delete_lines(&mut term, &mut host, 1);

    assert_visible_contents(&term, &["111", "555", "aaa", "   ", "bbb"]);
    assert_dirty_lines!(&term, &[1, 2, 3]);

    // expand the scroll region to fill the screen
    set_scroll_region(&mut term, &mut host, 0, 4);
    term.clean_dirty_lines();
    delete_lines(&mut term, &mut host, 1);

    assert_visible_contents(&term, &["111", "aaa", "   ", "bbb", "   "]);
    assert_dirty_lines!(&term, &[1, 2, 3, 4]);
}
