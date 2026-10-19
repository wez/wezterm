//! Tests for inline image protocol handling

use super::*;
use wezterm_cell::image::ImageDataType;
use wezterm_surface::change::ImageData;

/// Minimal base64 encoder so that the tests below can synthesize kitty
/// direct-transmission payloads without adding a dependency.
fn base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(ALPHABET[(n >> 18) as usize & 0x3f] as char);
        out.push(ALPHABET[(n >> 12) as usize & 0x3f] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[(n >> 6) as usize & 0x3f] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[n as usize & 0x3f] as char
        } else {
            '='
        });
    }
    out
}

/// Aggregate per-cell image attachment statistics over the visible screen:
/// (cells with images, total attachments, max attachments on a single cell,
/// number of distinct (image_id, placement_id) pairs).
fn image_attachment_stats(term: &mut TestTerm) -> (usize, usize, usize, usize) {
    let screen = term.screen_mut();
    let phys_rows = screen.physical_rows;
    let top = screen.visible_row_to_stable_row(0);
    let range = screen.stable_range(&(top..top + phys_rows as crate::StableRowIndex));
    let mut cells = 0;
    let mut total = 0;
    let mut max = 0;
    let mut pairs = std::collections::HashSet::new();
    for idx in range {
        let line = screen.line_mut(idx);
        for cell in line.visible_cells() {
            if let Some(images) = cell.attrs().images() {
                cells += 1;
                total += images.len();
                max = max.max(images.len());
                for im in &images {
                    pairs.insert((im.image_id(), im.placement_id()));
                }
            }
        }
    }
    (cells, total, max, pairs.len())
}

/// A client that sends no image id (`viu`, once per GIF frame) lands in the
/// anonymous id 0 bucket; re-placing must replace the attachment, not stack.
/// See <https://github.com/wezterm/wezterm/issues/7400>.
#[test]
fn kitty_anonymous_placement_reissue_does_not_accumulate() {
    let mut term = TestTerm::new(6, 10, 0);

    // 32x16 RGBA; at the 8x16 cell size of TestTerm we scale it to
    // 4 columns x 2 rows via c=/r=. a=T transmits and displays in one shot
    // with no image id at all, which is what viuer emits per frame.
    let rgba: Vec<u8> = std::iter::repeat([0x80u8, 0x80, 0x80, 0xff])
        .take(32 * 16)
        .flatten()
        .collect();
    let payload = base64_encode(&rgba);

    for _ in 0..20 {
        term.print("\x1b[H");
        term.print(format!(
            "\x1b_Ga=T,t=d,f=32,q=2,s=32,v=16,c=4,r=2,C=1;{}\x1b\\",
            payload
        ));
    }

    let (cells, total, max, pairs) = image_attachment_stats(&mut term);
    k9::assert_equal!(
        (cells, total, max, pairs),
        (8, 8, 1, 1),
        "after any number of anonymous re-placements each covered cell holds \
         exactly one attachment"
    );
}

/// Guards the fix above from over-scrubbing: several distinct images may
/// coexist under id 0, so `viu a.png; viu b.png` must leave both on screen.
#[test]
fn kitty_anonymous_placements_at_distinct_positions_coexist() {
    let mut term = TestTerm::new(6, 10, 0);

    let rgba: Vec<u8> = std::iter::repeat([0x80u8, 0x80, 0x80, 0xff])
        .take(32 * 16)
        .flatten()
        .collect();
    let payload = base64_encode(&rgba);

    for home in ["\x1b[H", "\x1b[4;1H"] {
        term.print(home);
        term.print(format!(
            "\x1b_Ga=T,t=d,f=32,q=2,s=32,v=16,c=4,r=2,C=1;{}\x1b\\",
            payload
        ));
    }

    let (cells, total, max, pairs) = image_attachment_stats(&mut term);
    k9::assert_equal!(
        (cells, total, max, pairs),
        (16, 16, 1, 1),
        "two anonymous images placed at different positions both remain attached"
    );
}

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

/// A 2x2 RGBA image, base64 encoded.
const TINY_RGBA_BASE64: &str = "AQID/wQFBv8HCAn/CgsM/w==";

/// The image data attached to the first cell that carries one.
fn first_image(term: &TestTerm) -> Arc<ImageData> {
    for line in term.screen().visible_lines().iter() {
        for cell in line.visible_cells() {
            if let Some(im) = cell
                .attrs()
                .images()
                .and_then(|images| images.into_iter().next())
            {
                return Arc::clone(im.image_data());
            }
        }
    }
    panic!("no image was attached to the screen");
}

/// An Rgba8 stores the hash of its pixels and `compute_hash` reports it without
/// recomputing, so a kitty frame transmission that edits those pixels in place
/// must leave the stored hash describing what is now there.
#[test]
fn kitty_frame_edit_keeps_the_stored_hash_current() {
    let mut term = TestTerm::new(3, 10, 0);

    // a=T: transmit and display, f=32: RGBA, s/v: 2x2 pixels.
    let seq = format!("\x1b_Ga=T,t=d,f=32,s=2,v=2,i=1;{}\x1b\\", TINY_RGBA_BASE64);
    term.advance_bytes(seq.as_bytes());
    let transmitted_hash = first_image(&term).data().compute_hash();

    // a=f: transmit a frame, r=1: edit frame 1 in place, painting the top left
    // pixel opaque red over the 0x01,0x02,0x03 it arrived with.
    term.advance_bytes(b"\x1b_Ga=f,t=d,f=32,s=1,v=1,i=1,r=1,x=0,y=0;/wAA/w==\x1b\\");

    let image = first_image(&term);
    let image = image.data();
    let edited_hash = image.compute_hash();

    match &*image {
        ImageDataType::Rgba8 { data, .. } => {
            // Asserted on the pixels rather than on the hash, so a blit that
            // did nothing cannot be mistaken for a hash that went stale.
            k9::assert_equal!(&data[0..4], &[0xff, 0x00, 0x00, 0xff][..]);
            k9::assert_equal!(edited_hash, ImageDataType::hash_bytes(data));
        }
        other => panic!("expected Rgba8, got {:?}", other),
    }

    assert_ne!(edited_hash, transmitted_hash);
}
