use super::*;

#[test]
fn test_vpa() {
    let mut term = TestTerm::new(3, 4, 0);
    term.assert_cursor_pos(0, 0, None);
    term.print("a\nb\nc");
    term.assert_cursor_pos(1, 2, None);
    term.print("\x1b[d");
    term.assert_cursor_pos(1, 0, None);
    term.print("\n\n");
    term.assert_cursor_pos(0, 2, None);

    // escapes are 1-based, so check that we're handling that
    // when we parse them!
    term.print("\x1b[2d");
    term.assert_cursor_pos(0, 1, None);
}

#[test]
fn test_ech() {
    let mut term = TestTerm::new(3, 4, 0);
    term.print("hey!wat?");
    term.cup(1, 0);
    term.print("\x1b[2X");
    assert_visible_contents(&term, &["h  !", "wat?", "    "]);
    // check how we handle overflowing the width
    term.print("\x1b[12X");
    assert_visible_contents(&term, &["h   ", "wat?", "    "]);
    term.print("\x1b[-12X");
    assert_visible_contents(&term, &["h   ", "wat?", "    "]);
}
