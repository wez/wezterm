//! Testing C1 control sequences

use super::*;

#[test]
fn test_ind() {
    let mut term = TestTerm::new(4, 4, 0);
    term.print("a\nb\x1bD");
    term.assert_cursor_pos(1, 2, None);
    assert_visible_contents(&term, &["a   ", "b   ", "    ", "    "]);
    term.print("\x1bD");
    term.assert_cursor_pos(1, 3, None);
    term.print("\x1bD");
    term.assert_cursor_pos(1, 3, None);
    assert_visible_contents(&term, &["b   ", "    ", "    ", "    "]);
}

#[test]
fn test_nel() {
    let mut term = TestTerm::new(4, 4, 0);
    term.print("a\nb\x1bE");
    term.assert_cursor_pos(0, 2, None);
    term.print("\x1bE");
    term.assert_cursor_pos(0, 3, None);
    term.print("\x1bE");
    term.assert_cursor_pos(0, 3, None);
    assert_visible_contents(&term, &["b   ", "    ", "    ", "    "]);
}
