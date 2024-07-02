use crate::termwindow::render::TripleLayerQuadAllocator;
use crate::termwindow::{UIItem, UIItemType};
use mux::pane::Pane;
use mux::tab::{PositionedPane, PositionedSplit, SplitDirection};
use std::sync::Arc;
use window::PixelUnit;

impl crate::TermWindow {
    pub fn paint_split(
        &mut self,
        layers: &mut TripleLayerQuadAllocator,
        split: &PositionedSplit,
        pane: &Arc<dyn Pane>,
    ) -> anyhow::Result<()> {
        let palette = pane.palette();
        let foreground = palette.split.to_linear();
        let cell_width = self.render_metrics.cell_size.width as f32;
        let cell_height = self.render_metrics.cell_size.height as f32;

        let border = self.get_os_border();
        let first_row_offset = if self.show_tab_bar && !self.config.tab_bar_at_bottom {
            self.tab_bar_pixel_height()?
        } else {
            0.
        } + border.top.get() as f32;

        let (padding_left, padding_top) = self.padding_left_top();

        let pos_y = split.top as f32 * cell_height + first_row_offset + padding_top;
        let pos_x = split.left as f32 * cell_width + padding_left + border.left.get() as f32;

        if split.direction == SplitDirection::Horizontal {
            self.filled_rectangle(
                layers,
                2,
                euclid::rect(
                    pos_x + (cell_width / 2.0),
                    pos_y - (cell_height / 2.0),
                    self.render_metrics.underline_height as f32,
                    (1. + split.size as f32) * cell_height,
                ),
                foreground,
            )?;
            self.ui_items.push(UIItem {
                x: border.left.get() as usize
                    + padding_left as usize
                    + (split.left * cell_width as usize),
                width: cell_width as usize,
                y: padding_top as usize
                    + first_row_offset as usize
                    + split.top * cell_height as usize,
                height: split.size * cell_height as usize,
                item_type: UIItemType::Split(split.clone()),
            });
        } else {
            self.filled_rectangle(
                layers,
                2,
                euclid::rect(
                    pos_x - (cell_width / 2.0),
                    pos_y + (cell_height / 2.0),
                    (1.0 + split.size as f32) * cell_width,
                    self.render_metrics.underline_height as f32,
                ),
                foreground,
            )?;
            self.ui_items.push(UIItem {
                x: border.left.get() as usize
                    + padding_left as usize
                    + (split.left * cell_width as usize),
                width: split.size * cell_width as usize,
                y: padding_top as usize
                    + first_row_offset as usize
                    + split.top * cell_height as usize,
                height: cell_height as usize,
                item_type: UIItemType::Split(split.clone()),
            });
        }

        Ok(())
    }

    pub fn paint_float_border(
        &mut self,
        pos: PositionedPane,
        layers: &mut TripleLayerQuadAllocator,
    ) -> anyhow::Result<()> {
        let (padding_left, padding_top) = self.padding_left_top();
        let config = self.config.float_pane_border.clone();
        let float_border = self.get_float_border();

        let os_border = self.get_os_border();
        let tab_bar_height = if self.show_tab_bar {
            self.tab_bar_pixel_height()?
        } else {
            0.
        };

        let (top_bar_height, bottom_bar_height) = if self.config.tab_bar_at_bottom {
            (0.0, tab_bar_height)
        } else {
            (tab_bar_height, 0.0)
        };
        let top_pixel_y = top_bar_height + padding_top + os_border.top.get() as f32;
        let cell_height = self.render_metrics.cell_size.height as f32;
        let cell_width = self.render_metrics.cell_size.width as f32;

        let background_rect: euclid::Rect<f32, PixelUnit> = {
            // We want to fill out to the edges of the splits
            let (x, width_delta) = if pos.left == 0 {
                (
                    0.,
                    padding_left + os_border.left.get() as f32 + (cell_width / 2.0),
                )
            } else {
                (
                    padding_left + os_border.left.get() as f32 - (cell_width / 2.0)
                        + (pos.left as f32 * cell_width),
                    cell_width,
                )
            };

            let (y, height_delta) = if pos.top == 0 {
                (
                    (top_pixel_y - padding_top),
                    padding_top + (cell_height / 2.0),
                )
            } else {
                (
                    top_pixel_y + (pos.top as f32 * cell_height) - (cell_height / 2.0),
                    cell_height,
                )
            };
            euclid::rect(
                x,
                y,
                // Go all the way to the right edge if we're right-most
                if pos.left + pos.width >= self.terminal_size.cols as usize {
                    self.dimensions.pixel_width as f32 - x
                } else {
                    (pos.width as f32 * cell_width) + width_delta
                },
                // Go all the way to the bottom if we're bottom-most
                if pos.top + pos.height >= self.terminal_size.rows as usize {
                    self.dimensions.pixel_height as f32 - y
                } else {
                    (pos.height as f32 * cell_height) + height_delta as f32
                },
            )
        };

        let pos_y = background_rect.origin.y - float_border.top.get() as f32;
        let pos_x = background_rect.origin.x - float_border.left.get() as f32;
        let pixel_width = background_rect.size.width + float_border.left.get() as f32;
        let pixel_height = background_rect.size.height + float_border.top.get() as f32;

        self.filled_rectangle(
            layers,
            2,
            euclid::rect(
                pos_x,
                pos_y,
                float_border.left.get() as f32,
                pixel_height + float_border.top.get() as f32,
            ),
            config.left_color.map(|c| c.to_linear()).unwrap_or(os_border.color),
        )?;
        self.filled_rectangle(
            layers,
            2,
            euclid::rect(
                pos_x + float_border.left.get() as f32,
                pos_y,
                pixel_width,
                float_border.top.get() as f32,
            ),
            config.top_color.map(|c| c.to_linear()).unwrap_or(os_border.color),
        )?;
        self.filled_rectangle(
            layers,
            2,
            euclid::rect(
                pos_x + float_border.left.get() as f32,
                pos_y + pixel_height,
                pixel_width,
                float_border.bottom.get() as f32,
            ),
            config.bottom_color.map(|c| c.to_linear()).unwrap_or(os_border.color),
        )?;
        self.filled_rectangle(
            layers,
            2,
            euclid::rect(
                pos_x + pixel_width,
                pos_y,
                float_border.right.get() as f32,
                pixel_height + float_border.top.get() as f32,
            ),
            config.right_color.map(|c| c.to_linear()).unwrap_or(os_border.color),
        )?;

        Ok(())
    }
}
