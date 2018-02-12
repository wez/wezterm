//! Testing C0 control characters

use super::*;

#[test]
fn test_bs() {
    let mut term = TestTerm::new(3, 4, 0);
    term.assert_cursor_pos(0, 0, None);
    term.print("\x08");
    term.assert_cursor_pos(0, 0, Some("cannot move left of the margin"));
    term.print("ab\x08");
    term.assert_cursor_pos(1, 0, None);
    // TODO: when we can set the left margin, we should test that here
}

#[test]
fn test_lf() {
    let mut term = TestTerm::new(3, 10, 0);
    term.print("hello\n");
    term.assert_cursor_pos(0, 1, Some("LF moves to new line"));
}

#[test]
fn test_cr() {
    let mut term = TestTerm::new(3, 10, 0);
    term.print("hello\r");
    term.assert_cursor_pos(0, 0, Some("CR moves to left margin on current line"));
    // TODO: when we can set the left margin, we should test that here
}

#[test]
fn test_tab() {
    let mut term = TestTerm::new(3, 25, 0);
    term.print("\t");
    term.assert_cursor_pos(8, 0, None);
    term.print("\t");
    term.assert_cursor_pos(16, 0, None);
    term.print("\t");
    term.assert_cursor_pos(24, 0, None);
    term.print("\t");
    term.assert_cursor_pos(24, 0, None);
}
