use mux::pane::Pane;
use wezterm_term::StableRowIndex;

pub struct ScrollHit {
    /// Offset from the top of the window in pixels
    pub top: usize,
    /// Height of the thumb, in pixels.
    pub height: usize,
}

impl ScrollHit {
    /// Compute the y-coordinate for the top of the scrollbar thumb
    /// and the height of the thumb and return them.
    pub fn thumb(
        pane: &dyn Pane,
        viewport: Option<StableRowIndex>,
        max_thumb_height: usize,
        min_thumb_size: usize,
    ) -> Self {
        let render_dims = pane.get_dimensions();

        let scroll_top = render_dims
            .physical_top
            .saturating_sub(viewport.unwrap_or(render_dims.physical_top))
            as f32;

        let scroll_size = render_dims.scrollback_rows as f32;

        let thumb_size = (render_dims.viewport_rows as f32 / scroll_size) * max_thumb_height as f32;

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
            (scroll_percent * (max_thumb_height.saturating_sub(thumb_size)) as f32).ceil() as usize;

        Self {
            top: thumb_top,
            height: thumb_size,
        }
    }

    /// Given a new thumb top coordinate (produced by dragging the thumb),
    /// compute the equivalent viewport offset.
    pub fn thumb_top_to_scroll_top(
        thumb_top: usize,
        pane: &dyn Pane,
        viewport: Option<StableRowIndex>,
        max_thumb_height: usize,
        min_thumb_size: usize,
    ) -> StableRowIndex {
        let thumb = Self::thumb(pane, viewport, max_thumb_height, min_thumb_size);
        let available_height = max_thumb_height - thumb.height;
        let scroll_percent = thumb_top.min(available_height) as f32 / available_height as f32;

        let render_dims = pane.get_dimensions();

        render_dims.scrollback_top.saturating_add(
            ((render_dims.physical_top - render_dims.scrollback_top) as f32 * scroll_percent)
                as StableRowIndex,
        )
    }
}
