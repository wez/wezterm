use super::*;

/// In this issue, the `CSI 2 P` sequence incorrectly removed two
/// cells from the line, leaving them effectively blank, when those
/// two cells should have been erased to the current background
/// color as set by `CSI 40 m`
#[test]
fn test_789() {
    let mut term = TestTerm::new(1, 8, 0);
    term.print("\x1b[40m\x1b[Kfoo\x1b[2P");

    k9::snapshot!(
        term.screen().visible_lines(),
        r#"
[
    Line {
        cells: V(
            VecStorage {
                cells: [
                    Cell {
                        text: "f",
                        width: 1,
                        attrs: CellAttributes {
                            attributes: 0,
                            intensity: Normal,
                            underline: None,
                            blink: None,
                            italic: false,
                            reverse: false,
                            strikethrough: false,
                            invisible: false,
                            wrapped: false,
                            overline: false,
                            semantic_type: Output,
                            foreground: Default,
                            background: PaletteIndex(
                                0,
                            ),
                            fat: None,
                        },
                    },
                    Cell {
                        text: "o",
                        width: 1,
                        attrs: CellAttributes {
                            attributes: 0,
                            intensity: Normal,
                            underline: None,
                            blink: None,
                            italic: false,
                            reverse: false,
                            strikethrough: false,
                            invisible: false,
                            wrapped: false,
                            overline: false,
                            semantic_type: Output,
                            foreground: Default,
                            background: PaletteIndex(
                                0,
                            ),
                            fat: None,
                        },
                    },
                    Cell {
                        text: "o",
                        width: 1,
                        attrs: CellAttributes {
                            attributes: 0,
                            intensity: Normal,
                            underline: None,
                            blink: None,
                            italic: false,
                            reverse: false,
                            strikethrough: false,
                            invisible: false,
                            wrapped: false,
                            overline: false,
                            semantic_type: Output,
                            foreground: Default,
                            background: PaletteIndex(
                                0,
                            ),
                            fat: None,
                        },
                    },
                    Cell {
                        text: " ",
                        width: 1,
                        attrs: CellAttributes {
                            attributes: 0,
                            intensity: Normal,
                            underline: None,
                            blink: None,
                            italic: false,
                            reverse: false,
                            strikethrough: false,
                            invisible: false,
                            wrapped: false,
                            overline: false,
                            semantic_type: Output,
                            foreground: Default,
                            background: PaletteIndex(
                                0,
                            ),
                            fat: None,
                        },
                    },
                    Cell {
                        text: " ",
                        width: 1,
                        attrs: CellAttributes {
                            attributes: 0,
                            intensity: Normal,
                            underline: None,
                            blink: None,
                            italic: false,
                            reverse: false,
                            strikethrough: false,
                            invisible: false,
                            wrapped: false,
                            overline: false,
                            semantic_type: Output,
                            foreground: Default,
                            background: PaletteIndex(
                                0,
                            ),
                            fat: None,
                        },
                    },
                    Cell {
                        text: " ",
                        width: 1,
                        attrs: CellAttributes {
                            attributes: 0,
                            intensity: Normal,
                            underline: None,
                            blink: None,
                            italic: false,
                            reverse: false,
                            strikethrough: false,
                            invisible: false,
                            wrapped: false,
                            overline: false,
                            semantic_type: Output,
                            foreground: Default,
                            background: PaletteIndex(
                                0,
                            ),
                            fat: None,
                        },
                    },
                    Cell {
                        text: " ",
                        width: 1,
                        attrs: CellAttributes {
                            attributes: 0,
                            intensity: Normal,
                            underline: None,
                            blink: None,
                            italic: false,
                            reverse: false,
                            strikethrough: false,
                            invisible: false,
                            wrapped: false,
                            overline: false,
                            semantic_type: Output,
                            foreground: Default,
                            background: PaletteIndex(
                                0,
                            ),
                            fat: None,
                        },
                    },
                    Cell {
                        text: " ",
                        width: 1,
                        attrs: CellAttributes {
                            attributes: 0,
                            intensity: Normal,
                            underline: None,
                            blink: None,
                            italic: false,
                            reverse: false,
                            strikethrough: false,
                            invisible: false,
                            wrapped: false,
                            overline: false,
                            semantic_type: Output,
                            foreground: Default,
                            background: PaletteIndex(
                                0,
                            ),
                            fat: None,
                        },
                    },
                ],
            },
        ),
        zones: [],
        seqno: 5,
        bits: LineBits(
            0x0,
        ),
        appdata: Mutex {
            data: None,
            poisoned: false,
            ..
        },
    },
]
"#
    );
}

#[test]
fn test_vpa() {
    let mut term = TestTerm::new(3, 4, 0);
    term.assert_cursor_pos(0, 0, None, Some(0));
    term.print("a\r\nb\r\nc");
    term.assert_cursor_pos(1, 2, None, None);
    term.print("\x1b[d");
    term.assert_cursor_pos(1, 0, None, None);
    term.print("\r\n\r\n");
    term.assert_cursor_pos(0, 2, None, None);

    // escapes are 1-based, so check that we're handling that
    // when we parse them!
    term.print("\x1b[2d");
    term.assert_cursor_pos(0, 1, None, None);
    term.print("\x1b[-2d");
    term.assert_cursor_pos(0, 1, None, Some(term.current_seqno() - 1));
}

#[test]
fn test_rep() {
    let mut term = TestTerm::new(3, 4, 0);
    term.print("h");
    term.cup(1, 0);
    term.print("\x1b[2ba");
    assert_visible_contents(&term, file!(), line!(), &["hhha", "", ""]);
}

#[test]
fn test_irm() {
    let mut term = TestTerm::new(3, 8, 0);
    term.print("foo");
    term.cup(0, 0);
    term.print("\x1b[4hBAR");
    assert_visible_contents(&term, file!(), line!(), &["BARfoo", "", ""]);
}

#[test]
fn test_ich() {
    let mut term = TestTerm::new(3, 4, 0);
    term.print("hey!wat?");
    term.cup(1, 0);
    term.print("\x1b[2@");
    assert_visible_contents(&term, file!(), line!(), &["h  e", "wat?", ""]);
    // check how we handle overflowing the width
    term.print("\x1b[12@");
    assert_visible_contents(&term, file!(), line!(), &["h   ", "wat?", ""]);
    term.print("\x1b[-12@");
    assert_visible_contents(&term, file!(), line!(), &["h   ", "wat?", ""]);
}

#[test]
fn test_ech() {
    let mut term = TestTerm::new(3, 4, 0);
    term.print("hey!wat?");
    term.cup(1, 0);
    term.print("\x1b[2X");
    assert_visible_contents(&term, file!(), line!(), &["h  !", "wat?", ""]);
    // check how we handle overflowing the width
    term.print("\x1b[12X");
    assert_visible_contents(&term, file!(), line!(), &["h   ", "wat?", ""]);
    term.print("\x1b[-12X");
    assert_visible_contents(&term, file!(), line!(), &["h   ", "wat?", ""]);
}

#[test]
fn test_dch() {
    let mut term = TestTerm::new(1, 12, 0);
    term.print("hello world");
    term.cup(1, 0);
    term.print("\x1b[P");
    assert_visible_contents(&term, file!(), line!(), &["hllo world"]);

    term.cup(4, 0);
    term.print("\x1b[2P");
    assert_visible_contents(&term, file!(), line!(), &["hlloorld"]);

    term.print("\x1b[-2P");
    assert_visible_contents(&term, file!(), line!(), &["hlloorld"]);
}

#[test]
fn test_cup() {
    let mut term = TestTerm::new(3, 4, 0);
    term.cup(1, 1);
    term.assert_cursor_pos(1, 1, None, None);
    term.cup(-1, -1);
    term.assert_cursor_pos(0, 0, None, None);
    term.cup(2, 2);
    term.assert_cursor_pos(2, 2, None, None);
    term.cup(-1, -1);
    term.assert_cursor_pos(0, 0, None, None);
    term.cup(500, 500);
    term.assert_cursor_pos(4, 2, None, None);
}

#[test]
fn test_hvp() {
    let mut term = TestTerm::new(3, 4, 0);
    term.hvp(1, 1);
    term.assert_cursor_pos(1, 1, None, None);
    term.hvp(-1, -1);
    term.assert_cursor_pos(0, 0, None, None);
    term.hvp(2, 2);
    term.assert_cursor_pos(2, 2, None, None);
    term.hvp(-1, -1);
    term.assert_cursor_pos(0, 0, None, None);
    term.hvp(500, 500);
    term.assert_cursor_pos(4, 2, None, None);
}

#[test]
fn test_dl() {
    let mut term = TestTerm::new(3, 1, 0);
    term.print("a\r\nb\r\nc");
    term.cup(0, 1);
    let seqno = term.current_seqno();
    term.delete_lines(1);
    assert_visible_contents(&term, file!(), line!(), &["a", "c", ""]);
    term.assert_cursor_pos(0, 1, None, Some(seqno));
    term.cup(0, 0);
    term.delete_lines(2);
    assert_visible_contents(&term, file!(), line!(), &["", "", ""]);
    term.print("1\r\n2\r\n3");
    term.cup(0, 1);
    term.delete_lines(-2);
    assert_visible_contents(&term, file!(), line!(), &["1", "2", "3"]);
}

#[test]
fn test_cha() {
    let mut term = TestTerm::new(3, 4, 0);
    term.cup(1, 1);
    term.assert_cursor_pos(1, 1, None, None);

    term.print("\x1b[G");
    term.assert_cursor_pos(0, 1, None, None);

    term.print("\x1b[2G");
    term.assert_cursor_pos(1, 1, None, None);

    term.print("\x1b[0G");
    term.assert_cursor_pos(0, 1, None, None);

    let seqno = term.current_seqno();
    term.print("\x1b[-1G");
    term.assert_cursor_pos(0, 1, None, Some(seqno));

    term.print("\x1b[100G");
    term.assert_cursor_pos(4, 1, None, None);
}

#[test]
fn test_ed() {
    let mut term = TestTerm::new(3, 3, 0);
    term.print("abc\r\ndef\r\nghi");
    term.cup(1, 2);
    term.print("\x1b[J");
    assert_visible_contents(&term, file!(), line!(), &["abc", "def", "g"]);

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
    line.fill_range(0..3, &Cell::new(' ', attr.clone()), SEQ_ZERO);
    assert_lines_equal(
        file!(),
        line!(),
        &term.screen().visible_lines(),
        &[line.clone(), line.clone(), line],
        Compare::TEXT | Compare::ATTRS,
    );
}

#[test]
fn test_ed_erase_scrollback() {
    let mut term = TestTerm::new(3, 3, 3);
    term.print("abc\r\ndef\r\nghi\r\n111\r\n222\r\na\x1b[3J");
    assert_all_contents(&term, file!(), line!(), &["111", "222", "a"]);
    term.print("b");
    assert_all_contents(&term, file!(), line!(), &["111", "222", "ab"]);
}
