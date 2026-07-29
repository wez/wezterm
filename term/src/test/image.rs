//! Tests for inline image protocol handling

use super::*;

/// A tiny but valid 11x11 PNG, base64 encoded.
/// Taken from the reproduction in <https://github.com/wezterm/wezterm/issues/6344>.
const TINY_PNG_BASE64: &str = "iVBORw0KGgoAAAANSUhEUgAAAAsAAAALCAYAAACprHcmAAAACXBIWXMAAAGKAAABigEzlzBYAAAAOUlEQVQYlZXOwQ0AMAzCQEdi7yaT0xWAN7JuDCac2PQKYxflycOoICOKtPIuqFCg4/LzKxiz6xjyAYh9DR1sLUN1AAAAAElFTkSuQmCC";

/// Feeding a Kitty graphics escape that requests a zero-sized placement (here `r=0,h=0`)
/// must not panic the terminal.
/// Prior to the fix for <https://github.com/wezterm/wezterm/issues/6344> this divided by zero
/// while computing the per-cell pixel deltas and took down the whole pane.
#[test]
fn kitty_zero_dimension_image_does_not_panic() {
    let mut term = TestTerm::new(3, 10, 0);

    // a=T: transmit and display, t=d: data is directly embedded,
    // f=100: PNG, r=0/h=0: zero rows / zero source height.
    let seq = format!("\x1b_Gr=0,h=0,a=T,t=d,f=100;{}\x1b\\", TINY_PNG_BASE64);
    term.print(seq.as_bytes());

    // The image is refused, so the cursor never moved;
    // Printing normal text and observing it confirms we recovered rather than crashing.
    term.print(b"ok");
    assert_visible_contents(&term, file!(), line!(), &["ok", "", ""]);
}

/// A well-formed Kitty graphic with non-zero dimensions should continue to be accepted.
/// The test passes as long as processing the image does not panic and the terminal remains usable.
#[test]
fn kitty_valid_image_is_accepted() {
    let mut term = TestTerm::new(3, 10, 0);

    let seq = format!("\x1b_Ga=T,t=d,f=100;{}\x1b\\", TINY_PNG_BASE64);
    term.print(seq.as_bytes());

    // Printing normal text and observing it shifted confirms the terminal is usable.
    term.print(b"ok");
    assert_visible_contents(&term, file!(), line!(), &["  ok", "", ""]);
}

/// When the pty has no pixel size, `cell_pixel_width`/`cell_pixel_height` are zero.
/// Displaying an image sized in cells (ie: without explicit `c=`/`r=`) must not divide by zero.
/// This is a distinct crash from the zero-dimension image above and is not caught by that guard.
/// See <https://github.com/wezterm/wezterm/issues/6344>.
#[test]
fn kitty_image_with_zero_pixel_dimensions_does_not_panic() {
    let mut term = Terminal::new(
        TerminalSize {
            rows: 3,
            cols: 80,
            // No pixel size!
            pixel_width: 0,
            pixel_height: 0,
            dpi: 0,
        },
        Arc::new(TestTermConfig { scrollback: 0 }),
        "WezTerm",
        "O_o",
        Box::new(Vec::new()),
    );

    // No `c=`/`r=`, so the placement is computed from the (zero) cell pixel
    // size, exercising the divide that previously panicked.
    let seq = format!("\x1b_Ga=T,t=d,f=100;{}\x1b\\", TINY_PNG_BASE64);
    term.advance_bytes(seq.as_bytes());

    // The image is refused, so the cursor never moved;
    // Printing normal text and observing it confirms we recovered rather than crashing.
    term.advance_bytes(b"ok");
    assert_visible_contents(&term, file!(), line!(), &["ok", "", ""]);
}
