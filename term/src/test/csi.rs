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
    term.print("\x1b[-2d");
    term.assert_cursor_pos(0, 1, None);
}

#[test]
fn test_ich() {
    let mut term = TestTerm::new(3, 4, 0);
    term.print("hey!wat?");
    term.cup(1, 0);
    term.print("\x1b[2@");
    assert_visible_contents(&term, &["h  ey!", "wat?", "    "]);
    // check how we handle overflowing the width
    term.print("\x1b[12@");
    assert_visible_contents(&term, &["h     ey!", "wat?", "    "]);
    term.print("\x1b[-12@");
    assert_visible_contents(&term, &["h     ey!", "wat?", "    "]);
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

#[test]
fn test_dch() {
    let mut term = TestTerm::new(1, 12, 0);
    term.print("hello world");
    term.cup(1, 0);
    term.print("\x1b[P");
    assert_visible_contents(&term, &["hllo world  "]);

    term.cup(4, 0);
    term.print("\x1b[2P");
    assert_visible_contents(&term, &["hlloorld    "]);

    term.print("\x1b[-2P");
    assert_visible_contents(&term, &["hlloorld    "]);
}

#[test]
fn test_cup() {
    let mut term = TestTerm::new(3, 4, 0);
    term.cup(1, 1);
    term.assert_cursor_pos(1, 1, None);
    term.cup(-1, -1);
    term.assert_cursor_pos(0, 0, None);
    term.cup(2, 2);
    term.assert_cursor_pos(2, 2, None);
    term.cup(-1, -1);
    term.assert_cursor_pos(0, 0, None);
    term.cup(500, 500);
    term.assert_cursor_pos(3, 2, None);
}

#[test]
fn test_hvp() {
    let mut term = TestTerm::new(3, 4, 0);
    term.hvp(1, 1);
    term.assert_cursor_pos(1, 1, None);
    term.hvp(-1, -1);
    term.assert_cursor_pos(0, 0, None);
    term.hvp(2, 2);
    term.assert_cursor_pos(2, 2, None);
    term.hvp(-1, -1);
    term.assert_cursor_pos(0, 0, None);
    term.hvp(500, 500);
    term.assert_cursor_pos(3, 2, None);
}

#[test]
fn test_dl() {
    let mut term = TestTerm::new(3, 1, 0);
    term.print("a\nb\nc");
    term.cup(0, 1);
    term.delete_lines(1);
    assert_visible_contents(&term, &["a", "c", " "]);
    term.assert_cursor_pos(0, 1, None);
    term.cup(0, 0);
    term.delete_lines(2);
    assert_visible_contents(&term, &[" ", " ", " "]);
    term.print("1\n2\n3");
    term.cup(0, 1);
    term.delete_lines(-2);
    assert_visible_contents(&term, &["1", "2", "3"]);
}

#[test]
fn test_cha() {
    let mut term = TestTerm::new(3, 4, 0);
    term.cup(1, 1);
    term.assert_cursor_pos(1, 1, None);

    term.print("\x1b[G");
    term.assert_cursor_pos(0, 1, None);

    term.print("\x1b[2G");
    term.assert_cursor_pos(1, 1, None);

    term.print("\x1b[0G");
    term.assert_cursor_pos(0, 1, None);

    term.print("\x1b[-1G");
    term.assert_cursor_pos(0, 1, None);

    term.print("\x1b[100G");
    term.assert_cursor_pos(3, 1, None);
}

#[test]
fn test_ed() {
    let mut term = TestTerm::new(3, 3, 0);
    term.print("abc\ndef\nghi");
    term.cup(0, 2);
    term.print("\x1b[J");
    assert_visible_contents(&term, &["abc", "def", "   "]);

    // Set background color to blue
    term.print("\x1b[44m");
    // Clear whole screen
    term.print("\x1b[2J");

    // Check that the background color paints all of the cells;
    // this is also known as BCE - Background Color Erase.
    let attr = CellAttributes::default()
        .set_background(color::AnsiColor::Navy)
        .clone();
    let mut line: Line = "   ".into();
    line.cells[0] = Cell::new(' ', attr.clone());
    line.cells[1] = Cell::new(' ', attr.clone());
    line.cells[2] = Cell::new(' ', attr.clone());
    assert_lines_equal(
        &term.screen().visible_lines(),
        &[line.clone(), line.clone(), line],
        Compare::TEXT | Compare::ATTRS,
    );
}
