use ::window::*;
use mux::pane::Pane;
use wezterm_term::StableRowIndex;

pub struct ScrollHit {
    /// Offset from the top of the window in pixels
    pub top: usize,
    /// Height of the thumb, in pixels.
    pub height: usize,
    /// Number of rows that correspond to the thumb in rows.
    /// This is normally == viewport height, but in the case
    /// where there are a sufficient number of rows of scrollback
    /// that the pixel height of the thumb would be too small,
    /// we will scale things in order to remain useful.
    pub rows: f32,
}

impl ScrollHit {
    /// Compute the y-coordinate for the top of the scrollbar thumb
    /// and the height of the thumb and return them.
    pub fn thumb(
        pane: &dyn Pane,
        viewport: Option<StableRowIndex>,
        dims: &Dimensions,
        tab_bar_height: f32,
        tab_bar_at_bottom: bool,
    ) -> Self {
        let max_thumb_height = dims.pixel_height as f32 - tab_bar_height;
        let render_dims = pane.get_dimensions();

        let scroll_top = render_dims
            .physical_top
            .saturating_sub(viewport.unwrap_or(render_dims.physical_top));

        let scroll_size = render_dims.scrollback_rows as f32;

        let thumb_size = (render_dims.viewport_rows as f32 / scroll_size) * max_thumb_height;

        const MIN_HEIGHT: f32 = 10.;
        let (thumb_size, rows) = if thumb_size < MIN_HEIGHT {
            (
                MIN_HEIGHT,
                render_dims.viewport_rows as f32 * MIN_HEIGHT / thumb_size,
            )
        } else {
            (thumb_size, render_dims.viewport_rows as f32)
        };

        let thumb_top = if tab_bar_at_bottom {
            0.
        } else {
            tab_bar_height
        } + (1.
            - (scroll_top as f32 + render_dims.viewport_rows as f32) / scroll_size)
            * max_thumb_height;

        let thumb_size = thumb_size.ceil() as usize;
        let thumb_top = thumb_top.ceil() as usize;

        Self {
            top: thumb_top,
            height: thumb_size,
            rows,
        }
    }

    /// Given a new thumb top coordinate (produced by dragging the thumb),
    /// compute the equivalent viewport offset.
    pub fn thumb_top_to_scroll_top(
        thumb_top: usize,
        pane: &dyn Pane,
        viewport: Option<StableRowIndex>,
        dims: &Dimensions,
        tab_bar_height: f32,
        tab_bar_at_bottom: bool,
    ) -> StableRowIndex {
        let render_dims = pane.get_dimensions();
        let thumb = Self::thumb(pane, viewport, dims, tab_bar_height, tab_bar_at_bottom);

        let rows_from_top = ((thumb_top as f32) / thumb.height as f32) * thumb.rows;

        render_dims
            .scrollback_top
            .saturating_add(rows_from_top as StableRowIndex)
    }
}
