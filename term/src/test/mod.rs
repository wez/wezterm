//! Various tests of the terminal model and escape sequence
//! processing routines.

use super::*;

macro_rules! assert_cursor_pos {
    ($term:expr, $x:expr, $y:expr) => {
        assert_cursor_pos!($term, $x, $y,
            "actual cursor (left) didn't match expected cursor (right)");
    };

    ($term:expr, $x:expr, $y:expr, $($reason:tt)*) => {
        {
            let cursor = $term.cursor_pos();
            assert_eq!(cursor, ($x, $y), $($reason)*);
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
                                             .map(|&(i, _)| i).collect();
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
                line.dirty , expect.dirty,
                "line {} dirty didn't match",
                idx,
            );
        }

        if compare.contains(Compare::ATTRS) {
            let line_attrs: Vec<_> = line.cells.iter().map(|c| c.attrs).collect();
            let expect_attrs: Vec<_> = expect.cells.iter().map(|c| c.attrs).collect();
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
fn set_mode(term: &mut Terminal, mode: &str, enable: bool) {
    term.advance_bytes(CSI);
    term.advance_bytes(mode);
    term.advance_bytes(if enable { b"h" } else { b"l" });
}

fn cup(term: &mut Terminal, col: isize, row: isize) {
    term.advance_bytes(CSI);
    term.advance_bytes(format!("{};{}H", row + 1, col + 1));
}

fn erase_in_display(term: &mut Terminal, erase: DisplayErase) {
    term.advance_bytes(CSI);
    let num = match erase {
        DisplayErase::Below => 0,
        DisplayErase::Above => 1,
        DisplayErase::All => 2,
        DisplayErase::SavedLines => 3,
    };
    term.advance_bytes(format!("{}J", num));
}

fn erase_in_line(term: &mut Terminal, erase: LineErase) {
    term.advance_bytes(CSI);
    let num = match erase {
        LineErase::ToRight => 0,
        LineErase::ToLeft => 1,
        LineErase::All => 2,
    };
    term.advance_bytes(format!("{}K", num));
}

bitflags! {
    struct Compare : u8{
        const TEXT = 1;
        const ATTRS = 2;
        const DIRTY = 3;
    }
}


/// Asserts that the visible lines of the terminal have the
/// same cell contents.  The cells must exactly match.
#[allow(dead_code)]
fn assert_visible_lines(term: &Terminal, expect_lines: &[Line]) {
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
    let screen = term.screen();

    let expect: Vec<Line> = expect_lines.iter().map(|s| (*s).into()).collect();

    assert_lines_equal(screen.visible_lines(), &expect, Compare::TEXT);
}

#[test]
fn basic_output() {
    let mut term = Terminal::new(5, 10, 0);

    cup(&mut term, 1, 1);
    term.advance_bytes("hello, world!");
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

    erase_in_display(&mut term, DisplayErase::Above);
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

    cup(&mut term, 2, 2);
    erase_in_line(&mut term, LineErase::ToRight);
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

    erase_in_line(&mut term, LineErase::ToLeft);
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

    term.advance_bytes("fooo.");
    assert_visible_contents(&term, &["foo", "o. "]);
    assert_cursor_pos!(&term, 2, 1);
    assert_dirty_lines!(&term, &[0, 1]);

    cup(&mut term, 0, 1);
    term.clean_dirty_lines();
    term.advance_bytes("\x08");
    assert_cursor_pos!(&term, 0, 1, "BS doesn't change the line");
    assert_dirty_lines!(&term, &[1]);
    term.clean_dirty_lines();

    cup(&mut term, 0, 0);
    assert_dirty_lines!(&term, &[0, 1], "cursor movement dirties old and new lines");
}
