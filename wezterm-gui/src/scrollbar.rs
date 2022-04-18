use mux::pane::{Pane, PaneId};
use wezterm_term::StableRowIndex;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct ScrollThumb {
    /// Offset from the top of the scroll area, in pixels.
    pub top: usize,
    /// Height of the thumb, in pixels.
    pub height: usize,
    /// Offset from the top of the window, in pixels.
    pub scrollbar_top: usize,
    /// Height of the scroll bar / scroll area, in pixels.
    pub scrollbar_height: usize,
    /// Pane id associated with this scroll thumb
    pub pane_id: PaneId,
}

impl ScrollThumb {
    /// Create a scroll bar thumb by calculating its parameters from window
    /// and pane info.
    pub fn new(
        pane: &dyn Pane,
        viewport: Option<StableRowIndex>,
        scrollbar_top: usize,
        scrollbar_height: usize,
        min_thumb_size: usize,
    ) -> Self {
        let render_dims = pane.get_dimensions();

        let scroll_top = render_dims
            .physical_top
            .saturating_sub(viewport.unwrap_or(render_dims.physical_top))
            as f32;

        let scroll_size = render_dims.scrollback_rows as f32;

        let thumb_size = (render_dims.viewport_rows as f32 / scroll_size) * scrollbar_height as f32;

        let min_thumb_size = min_thumb_size as f32;
        let thumb_size = if thumb_size < min_thumb_size {
            min_thumb_size
        } else {
            thumb_size
        }
        .ceil() as usize;

        let scroll_percent =
            1.0 - (scroll_top / (render_dims.physical_top - render_dims.scrollback_top) as f32);
        let thumb_top =
            (scroll_percent * (scrollbar_height.saturating_sub(thumb_size)) as f32).ceil() as usize;

        Self {
            top: thumb_top,
            height: thumb_size,
            scrollbar_top,
            scrollbar_height,
            pane_id: pane.pane_id(),
        }
    }

    /// Given a cursor y offset from the window top and its offset from the
    /// thumb button, calculate the equivalent scroll percentage. The percentage
    /// goes from 0.0 when the thumb button is at the top of the scroll bar to
    /// 1.0 when it's at the bottom.
    pub fn scroll_percentage(&self, cursor_y: isize, thumb_offset: isize) -> f32 {
        let effective_y = (cursor_y - thumb_offset - self.scrollbar_top as isize).max(0);
        let available_height = self.scrollbar_height - self.height;
        (effective_y as f32 / available_height as f32).min(1.)
    }
}
