use super::*;

/// Test basic dragging to select some text
#[test]
fn drag_selection() {
    let mut term = TestTerm::new(3, 12, 0);
    term.print("hello world\n");
    assert_visible_contents(&term, &["hello world ", "            ", "            "]);

    term.drag_select(1, 0, 4, 0);
    assert_eq!(term.get_clipboard().unwrap(), "ello");

    // Now check that we respect double-width boundaries reasonably sanely;
    // here we're dragging from the middle of the skull emoji
    term.print("\u{1F480}skull\n");
    assert_visible_contents(
        &term,
        &["hello world ", "\u{1F480}skull     ", "            "],
    );
    term.drag_select(1, 1, 5, 1);
    assert_eq!(term.get_clipboard().unwrap(), "skul");

    // Let's include the start of it this time
    term.drag_select(0, 1, 5, 1);
    assert_eq!(term.get_clipboard().unwrap(), "\u{1F480}skul");

    // Multi-line selection
    term.drag_select(1, 0, 6, 1);
    assert_eq!(term.get_clipboard().unwrap(), "ello world\n\u{1F480}skull");

    // This next one drags off the bottom; this is technically out of bounds
    // but we want to make sure we handle this without panicking.  See the
    // comment in TerminalState::mouse_event for more info.
    term.drag_select(0, 0, 15, 3);
    assert_eq!(
        term.get_clipboard().unwrap(),
        "hello world\n\u{1F480}skull\n"
    );

    term.drag_select(6, 0, 3, 1);
    assert_eq!(term.get_clipboard().unwrap(), "world\n\u{1F480}sk");
}

/// Test double click to select a word
#[test]
fn double_click_selection() {
    let mut term = TestTerm::new(3, 10, 0);
    term.print("hello world");

    term.click_n(1, 0, MouseButton::Left, 2);

    assert_eq!(term.get_clipboard().unwrap(), "hello");
}

/// Test triple click to select a line
#[test]
fn triple_click_selection() {
    let mut term = TestTerm::new(3, 10, 0);
    term.print("hello world");
    assert_visible_contents(&term, &["hello worl", "d         ", "          "]);
    term.click_n(1, 0, MouseButton::Left, 3);

    assert_eq!(term.get_clipboard().unwrap(), "hello worl");
}
