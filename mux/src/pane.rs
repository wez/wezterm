use crate::domain::DomainId;
use crate::renderable::*;
use crate::Mux;
use async_trait::async_trait;
use config::keyassignment::ScrollbackEraseMode;
use downcast_rs::{impl_downcast, Downcast};
use portable_pty::PtySize;
use rangeset::RangeSet;
use serde::{Deserialize, Serialize};
use std::cell::RefMut;
use std::ops::Range;
use std::sync::{Arc, Mutex};
use termwiz::surface::Line;
use url::Url;
use wezterm_term::color::ColorPalette;
use wezterm_term::{Clipboard, KeyCode, KeyModifiers, MouseEvent, SemanticZone, StableRowIndex};

static PANE_ID: ::std::sync::atomic::AtomicUsize = ::std::sync::atomic::AtomicUsize::new(0);
pub type PaneId = usize;

pub fn alloc_pane_id() -> PaneId {
    PANE_ID.fetch_add(1, ::std::sync::atomic::Ordering::Relaxed)
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SearchResult {
    pub start_y: StableRowIndex,
    /// The cell index into the line of the start of the match
    pub start_x: usize,
    pub end_y: StableRowIndex,
    /// The cell index into the line of the end of the match
    pub end_x: usize,
}

pub use config::keyassignment::Pattern;

const PASTE_CHUNK_SIZE: usize = 1024;

struct Paste {
    pane_id: PaneId,
    text: String,
    offset: usize,
}

fn schedule_next_paste(paste: &Arc<Mutex<Paste>>) {
    let paste = Arc::clone(paste);
    promise::spawn::spawn(async move {
        let mut locked = paste.lock().unwrap();
        let mux = Mux::get().unwrap();
        let pane = mux.get_pane(locked.pane_id).unwrap();

        let remain = locked.text.len() - locked.offset;
        let mut chunk = remain.min(PASTE_CHUNK_SIZE);

        // Make sure we chunk at a char boundary, otherwise the
        // slice operation below will panic
        while !locked.text.is_char_boundary(locked.offset + chunk) && chunk < remain {
            chunk += 1;
        }
        let text_slice = &locked.text[locked.offset..locked.offset + chunk];
        pane.send_paste(text_slice).unwrap();

        if chunk < remain {
            // There is more to send
            locked.offset += chunk;
            schedule_next_paste(&paste);
        }
    })
    .detach();
}

/// A Pane represents a view on a terminal
#[async_trait(?Send)]
pub trait Pane: Downcast {
    fn pane_id(&self) -> PaneId;

    /// Returns the 0-based cursor position relative to the top left of
    /// the visible screen
    fn get_cursor_position(&self) -> StableCursorPosition;

    /// Given a range of lines, return the subset of those lines that
    /// have their dirty flag set to true.
    fn get_dirty_lines(&self, lines: Range<StableRowIndex>) -> RangeSet<StableRowIndex>;

    /// Returns a set of lines from the scrollback or visible portion of
    /// the display.  The lines are indexed using StableRowIndex, which
    /// can be invalidated if the scrollback is busy, or when switching
    /// to the alternate screen.
    /// To deal with this, this function will adjust the input so that
    /// a range that has been scrolled off the top will return the top
    /// n rows of the scrollback (where n is the size of the input range),
    /// or the bottom n rows of the scrollback when switching to the alt
    /// screen and the index would go off the bottom.
    /// Because of this, we also return the adjusted StableRowIndex for
    /// the first row in the range.
    ///
    /// For each line, if it was dirty in the backing data, then the dirty
    /// flag will be cleared in the backing data.  The returned line will
    /// have its dirty bit set appropriately.
    fn get_lines(&self, lines: Range<StableRowIndex>) -> (StableRowIndex, Vec<Line>);

    /// Returns render related dimensions
    fn get_dimensions(&self) -> RenderableDimensions;

    fn get_title(&self) -> String;
    fn send_paste(&self, text: &str) -> anyhow::Result<()>;
    fn reader(&self) -> anyhow::Result<Box<dyn std::io::Read + Send>>;
    fn writer(&self) -> RefMut<dyn std::io::Write>;
    fn resize(&self, size: PtySize) -> anyhow::Result<()>;
    /// Called as a hint that the pane is being resized as part of
    /// a zoom-to-fill-all-the-tab-space operation.
    fn set_zoomed(&self, _zoomed: bool) {}
    fn key_down(&self, key: KeyCode, mods: KeyModifiers) -> anyhow::Result<()>;
    fn mouse_event(&self, event: MouseEvent) -> anyhow::Result<()>;
    fn advance_bytes(&self, buf: &[u8]);
    fn is_dead(&self) -> bool;
    fn kill(&self) {}
    fn palette(&self) -> ColorPalette;
    fn domain_id(&self) -> DomainId;

    fn erase_scrollback(&self, _erase_mode: ScrollbackEraseMode) {}

    /// Called to advise on whether this tab has focus
    fn focus_changed(&self, _focused: bool) {}

    /// Performs a search.
    /// If the result is empty then there are no matches.
    /// Otherwise, the result shall contain all possible matches.
    async fn search(&self, _pattern: Pattern) -> anyhow::Result<Vec<SearchResult>> {
        Ok(vec![])
    }

    /// Retrieve the set of semantic zones
    fn get_semantic_zones(&self) -> anyhow::Result<Vec<SemanticZone>> {
        Ok(vec![])
    }

    /// Returns true if the terminal has grabbed the mouse and wants to
    /// give the embedded application a chance to process events.
    /// In practice this controls whether the gui will perform local
    /// handling of clicks.
    fn is_mouse_grabbed(&self) -> bool;
    fn is_alt_screen_active(&self) -> bool;

    fn set_clipboard(&self, _clipboard: &Arc<dyn Clipboard>) {}

    fn get_current_working_dir(&self) -> Option<Url>;

    fn trickle_paste(&self, text: String) -> anyhow::Result<()> {
        if text.len() <= PASTE_CHUNK_SIZE {
            // Send it all now
            self.send_paste(&text)?;
        } else {
            // It's pretty heavy, so we trickle it into the pty
            self.send_paste(&text[0..PASTE_CHUNK_SIZE])?;

            let paste = Arc::new(Mutex::new(Paste {
                pane_id: self.pane_id(),
                text,
                offset: PASTE_CHUNK_SIZE,
            }));
            schedule_next_paste(&paste);
        }
        Ok(())
    }
}
impl_downcast!(Pane);
