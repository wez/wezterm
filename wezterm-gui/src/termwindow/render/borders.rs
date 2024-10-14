use crate::quad::TripleLayerQuadAllocator;
use crate::utilsprites::RenderMetrics;
use ::window::ULength;
use config::{ConfigHandle, Dimension, DimensionContext, FloatingPaneBorderConfig, PixelUnit};
use mux::tab::PositionedPane;
use window::parameters::Border;

impl crate::TermWindow {
    pub fn paint_window_borders(
        &mut self,
        layers: &mut TripleLayerQuadAllocator,
    ) -> anyhow::Result<()> {
        let border_dimensions = self.get_os_border();

        if border_dimensions.top.get() > 0
            || border_dimensions.bottom.get() > 0
            || border_dimensions.left.get() > 0
            || border_dimensions.right.get() > 0
        {
            let height = self.dimensions.pixel_height as f32;
            let width = self.dimensions.pixel_width as f32;

            let border_top = border_dimensions.top.get() as f32;
            if border_top > 0.0 {
                self.filled_rectangle(
                    layers,
                    1,
                    euclid::rect(0.0, 0.0, width, border_top),
                    self.config
                        .window_frame
                        .border_top_color
                        .map(|c| c.to_linear())
                        .unwrap_or(border_dimensions.color),
                )?;
            }

            let border_left = border_dimensions.left.get() as f32;
            if border_left > 0.0 {
                self.filled_rectangle(
                    layers,
                    1,
                    euclid::rect(0.0, 0.0, border_left, height),
                    self.config
                        .window_frame
                        .border_left_color
                        .map(|c| c.to_linear())
                        .unwrap_or(border_dimensions.color),
                )?;
            }

            let border_bottom = border_dimensions.bottom.get() as f32;
            if border_bottom > 0.0 {
                self.filled_rectangle(
                    layers,
                    1,
                    euclid::rect(0.0, height - border_bottom, width, height),
                    self.config
                        .window_frame
                        .border_bottom_color
                        .map(|c| c.to_linear())
                        .unwrap_or(border_dimensions.color),
                )?;
            }

            let border_right = border_dimensions.right.get() as f32;
            if border_right > 0.0 {
                self.filled_rectangle(
                    layers,
                    1,
                    euclid::rect(width - border_right, 0.0, border_right, height),
                    self.config
                        .window_frame
                        .border_right_color
                        .map(|c| c.to_linear())
                        .unwrap_or(border_dimensions.color),
                )?;
            }
        }

        Ok(())
    }

    pub fn compute_background_rect(
        &self,
        pos: &PositionedPane,
        padding_left: f32,
        padding_top: f32,
        border: &Border,
        top_pixel_y: f32,
        cell_width: f32,
        cell_height: f32,
    ) -> euclid::Rect<f32, window::PixelUnit> {
        let (x, width_delta) = if pos.left == 0 {
            (
                0.,
                padding_left + border.left.get() as f32 + (cell_width / 2.0),
            )
        } else {
            (
                padding_left + border.left.get() as f32 - (cell_width / 2.0)
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
            // Width calculation
            if pos.left + pos.width >= self.terminal_size.cols as usize {
                self.dimensions.pixel_width as f32 - x
            } else {
                (pos.width as f32 * cell_width) + width_delta
            },
            // Height calculation
            if pos.top + pos.height >= self.terminal_size.rows as usize {
                self.dimensions.pixel_height as f32 - y
            } else {
                (pos.height as f32 * cell_height) + height_delta
            },
        )
    }

    pub fn paint_floating_pane_border(
        &mut self,
        pos: PositionedPane,
        layers: &mut TripleLayerQuadAllocator,
    ) -> anyhow::Result<()> {
        let (padding_left, padding_top) = self.padding_left_top();
        let config = self.config.floating_pane_border.clone();
        let floating_pane_border = self.get_floating_pane_border();

        let os_border = self.get_os_border();
        let tab_bar_height = if self.show_tab_bar {
            self.tab_bar_pixel_height()?
        } else {
            0.
        };

        let (top_bar_height, _bottom_bar_height) = if self.config.tab_bar_at_bottom {
            (0.0, tab_bar_height)
        } else {
            (tab_bar_height, 0.0)
        };
        let top_pixel_y = top_bar_height + padding_top + os_border.top.get() as f32;
        let cell_height = self.render_metrics.cell_size.height as f32;
        let cell_width = self.render_metrics.cell_size.width as f32;

        let background_rect = self.compute_background_rect(&pos,
            padding_left, padding_top, &os_border, top_pixel_y, cell_width, cell_height);

        let pos_y = background_rect.origin.y - floating_pane_border.top.get() as f32;
        let pos_x = background_rect.origin.x - floating_pane_border.left.get() as f32;
        let pixel_width = background_rect.size.width + floating_pane_border.left.get() as f32;
        let pixel_height = background_rect.size.height + floating_pane_border.top.get() as f32;

        self.filled_rectangle(
            layers,
            2,
            euclid::rect(
                pos_x,
                pos_y,
                floating_pane_border.left.get() as f32,
                pixel_height + floating_pane_border.top.get() as f32,
            ),
            config.left_color.map(|c| c.to_linear()).unwrap_or(os_border.color),
        )?;
        self.filled_rectangle(
            layers,
            2,
            euclid::rect(
                pos_x + floating_pane_border.left.get() as f32,
                pos_y,
                pixel_width,
                floating_pane_border.top.get() as f32,
            ),
            config.top_color.map(|c| c.to_linear()).unwrap_or(os_border.color),
        )?;
        self.filled_rectangle(
            layers,
            2,
            euclid::rect(
                pos_x + floating_pane_border.left.get() as f32,
                pos_y + pixel_height,
                pixel_width,
                floating_pane_border.bottom.get() as f32,
            ),
            config.bottom_color.map(|c| c.to_linear()).unwrap_or(os_border.color),
        )?;
        self.filled_rectangle(
            layers,
            2,
            euclid::rect(
                pos_x + pixel_width,
                pos_y,
                floating_pane_border.right.get() as f32,
                pixel_height + floating_pane_border.top.get() as f32,
            ),
            config.right_color.map(|c| c.to_linear()).unwrap_or(os_border.color),
        )?;

        Ok(())
    }

    fn apply_config_to_border(
        border: &mut Border,
        left_width: Dimension,
        right_width: Dimension,
        top_height: Dimension,
        bottom_height: Dimension,
        dimensions: &crate::Dimensions,
        render_metrics: &RenderMetrics,
    ) {
        border.left += ULength::new(
            left_width
                .evaluate_as_pixels(DimensionContext {
                    dpi: dimensions.dpi as f32,
                    pixel_max: dimensions.pixel_width as f32,
                    pixel_cell: render_metrics.cell_size.width as f32,
                })
                .ceil() as usize,
        );
        border.right += ULength::new(
            right_width
                .evaluate_as_pixels(DimensionContext {
                    dpi: dimensions.dpi as f32,
                    pixel_max: dimensions.pixel_width as f32,
                    pixel_cell: render_metrics.cell_size.width as f32,
                })
                .ceil() as usize,
        );
        border.top += ULength::new(
            top_height
                .evaluate_as_pixels(DimensionContext {
                    dpi: dimensions.dpi as f32,
                    pixel_max: dimensions.pixel_height as f32,
                    pixel_cell: render_metrics.cell_size.height as f32,
                })
                .ceil() as usize,
        );
        border.bottom += ULength::new(
            bottom_height
                .evaluate_as_pixels(DimensionContext {
                    dpi: dimensions.dpi as f32,
                    pixel_max: dimensions.pixel_height as f32,
                    pixel_cell: render_metrics.cell_size.height as f32,
                })
                .ceil() as usize,
        );
    }

    pub fn get_os_border_impl(
        os_parameters: &Option<window::parameters::Parameters>,
        config: &ConfigHandle,
        dimensions: &crate::Dimensions,
        render_metrics: &RenderMetrics,
    ) -> window::parameters::Border {
        let mut border = os_parameters
            .as_ref()
            .and_then(|p| p.border_dimensions.clone())
            .unwrap_or_default();

        Self::apply_config_to_border(
            &mut border,
            config.window_frame.border_left_width,
            config.window_frame.border_right_width,
            config.window_frame.border_top_height,
            config.window_frame.border_bottom_height,
            dimensions,
            render_metrics
        );

        border
    }

    fn get_floating_pane_border_impl(
        dimensions: &crate::Dimensions,
        render_metrics: &RenderMetrics,
        border_config: &FloatingPaneBorderConfig
    ) -> Border {
        let mut border= Border::default();
        Self::apply_config_to_border(
            &mut border,
            border_config.left_width,
            border_config.right_width,
            border_config.top_height,
            border_config.bottom_height,
            dimensions,
            render_metrics
        );
        border
    }

    fn get_floating_pane_border(&self) -> Border {
        Self::get_floating_pane_border_impl(
            &self.dimensions,
            &self.render_metrics,
            &self.config.floating_pane_border
        )
    }

    pub fn get_os_border(&self) -> window::parameters::Border {
        Self::get_os_border_impl(
            &self.os_parameters,
            &self.config,
            &self.dimensions,
            &self.render_metrics,
        )
    }
}
