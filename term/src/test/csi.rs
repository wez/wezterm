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

fn ed2_scroll_term(height: usize, width: usize, scrollback: usize) -> TestTerm {
    TestTerm::new_with_config(
        height,
        width,
        TestTermConfig {
            scrollback,
            erase_display_scrolls_into_scrollback: true,
        },
    )
}

/// With erase_display_scrolls_into_scrollback enabled, `CSI 2 J` moves what
/// was on screen into the scrollback instead of discarding it.
#[test]
fn erase_display_scrolls_screen_into_scrollback() {
    let mut term = ed2_scroll_term(4, 8, 10);
    term.print("one\r\ntwo\r\nthree\r\n");
    term.cup(0, 0);
    let seqno = term.current_seqno();
    term.erase_in_display(EraseInDisplay::EraseDisplay);

    assert_visible_contents(&term, file!(), line!(), &["", "", "", ""]);
    assert_all_contents(
        &term,
        file!(),
        line!(),
        &["one", "two", "three", "", "", "", ""],
    );
    term.assert_cursor_pos(0, 0, Some("2J doesn't move the cursor"), Some(seqno));
}

/// Only the rows up to the last row that holds something are scrolled, so
/// clearing a mostly-empty screen doesn't push a screenful of blank rows
/// into the scrollback and evict real history.
#[test]
fn erase_display_scroll_does_not_pad_scrollback() {
    let mut term = ed2_scroll_term(4, 8, 10);
    term.print("hi");
    term.erase_in_display(EraseInDisplay::EraseDisplay);

    assert_all_contents(&term, file!(), line!(), &["hi", "", "", "", ""]);
}

/// Clearing a screen that holds nothing at all adds nothing to the
/// scrollback.
#[test]
fn erase_display_scroll_of_empty_screen_is_a_no_op() {
    let mut term = ed2_scroll_term(4, 8, 10);
    term.erase_in_display(EraseInDisplay::EraseDisplay);

    assert_all_contents(&term, file!(), line!(), &["", "", "", ""]);
}

/// A blank row between two rows that hold something is part of the screen
/// layout, so it is preserved rather than skipped.
#[test]
fn erase_display_scroll_preserves_interior_blank_rows() {
    let mut term = ed2_scroll_term(4, 8, 10);
    term.print("one\r\n\r\nthree");
    term.erase_in_display(EraseInDisplay::EraseDisplay);

    assert_all_contents(
        &term,
        file!(),
        line!(),
        &["one", "", "three", "", "", "", ""],
    );
}

/// `CSI 2 J` erases to the current background colour; scrolling first must
/// not leave rows painted with the previous attributes.
#[test]
fn erase_display_scroll_still_erases_to_current_background() {
    let mut term = ed2_scroll_term(4, 8, 10);
    term.print("hi");
    term.print("\x1b[41m");
    term.erase_in_display(EraseInDisplay::EraseDisplay);

    let mut cells = 0;
    for line in term.screen().visible_lines() {
        for cell in line.visible_cells() {
            cells += 1;
            k9::assert_equal!(
                cell.attrs().background(),
                wezterm_cell::color::ColorAttribute::PaletteIndex(1),
                "every erased cell takes the current background"
            );
        }
    }
    k9::assert_equal!(cells, 4 * 8);

    assert_all_contents(
        &term,
        file!(),
        line!(),
        &["hi", "        ", "        ", "        ", "        "],
    );
}

/// The option only applies to `CSI 2 J`; the other erase-in-display forms
/// are untouched.
#[test]
fn erase_display_scroll_does_not_apply_to_partial_erases() {
    let mut term = ed2_scroll_term(4, 8, 10);
    term.print("one\r\ntwo\r\nthree");
    term.cup(0, 1);
    term.erase_in_display(EraseInDisplay::EraseToEndOfDisplay);

    assert_all_contents(
        &term,
        file!(),
        line!(),
        &["one", "        ", "        ", ""],
    );
}

/// Regression pin: with the option left at its default, `CSI 2 J` erases in
/// place, as the spec describes. Passes before and after this change.
#[test]
fn erase_display_default_erases_in_place() {
    let mut term = TestTerm::new(4, 8, 10);
    term.print("one\r\ntwo\r\nthree");
    term.erase_in_display(EraseInDisplay::EraseDisplay);

    assert_all_contents(
        &term,
        file!(),
        line!(),
        &["        ", "        ", "        ", ""],
    );
}
