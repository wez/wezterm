//! Various tests of the terminal model and escape sequence
//! processing routines.

use super::*;

fn set_mode(term: &mut Terminal, mode: &str, enable: bool) {
    term.advance_bytes(CSI);
    term.advance_bytes(mode);
    term.advance_bytes(if enable { b"h" } else { b"l" });
}

fn cup(term: &mut Terminal, row: isize, col: isize) {
    term.advance_bytes(CSI);
    term.advance_bytes(format!("{};{}H", row, col));
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

/// Asserts that the visible lines of the terminal have the
/// same cell contents.  The cells must exactly match.
fn assert_visible_lines(term: &Terminal, expect_lines: &[Line]) {
    let screen = term.screen();
    let lines = screen.visible_lines();

    assert!(
        lines.len() == expect_lines.len(),
        "expectation has wrong number of lines"
    );

    let mut expect_iter = expect_lines.iter();

    for (idx, line) in lines.iter().enumerate() {
        let expect = expect_iter.next().unwrap();

        assert!(
            expect.cells == line.cells,
            "line {} was {:?} but expected {:?}",
            idx,
            line.cells,
            expect.cells
        );
    }
}

/// Asserts that the visible lines of the terminal have the
/// same character contents as the expected lines.
/// The other cell attributes are not compared; this is
/// a convenience for writing visually understandable tests.
fn assert_visible_contents(term: &Terminal, expect_lines: &[&str]) {
    let screen = term.screen();
    let lines = screen.visible_lines();

    assert!(
        lines.len() == expect_lines.len(),
        "expectation has wrong number of lines"
    );

    let mut expect_iter = expect_lines.iter();

    for (idx, line) in lines.iter().enumerate() {
        let expect = expect_iter.next().unwrap();
        let line_str = line.as_str();

        assert!(
            &line_str == expect,
            "line {} was {:?} but expected {:?}",
            idx,
            line_str,
            expect
        );
    }
}

#[test]
fn basic_output() {
    let mut term = Terminal::new(5, 10, 0);

    cup(&mut term, 2, 2);
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
}
