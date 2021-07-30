use crate::utilsprites::RenderMetrics;
use ::window::{Dimensions, WindowOps};
use config::ConfigHandle;
use mux::Mux;
use portable_pty::PtySize;
use std::rc::Rc;
use wezterm_font::FontConfiguration;

#[derive(Debug, Clone, Copy)]
pub struct RowsAndCols {
    rows: usize,
    cols: usize,
}

impl super::TermWindow {
    pub fn resize(&mut self, dimensions: Dimensions, is_full_screen: bool) {
        log::trace!(
            "resize event, current cells: {:?}, new dims: {:?} is_full_screen:{}",
            self.current_cell_dimensions(),
            dimensions,
            is_full_screen,
        );
        if dimensions.pixel_width == 0 || dimensions.pixel_height == 0 {
            // on windows, this can happen when minimizing the window.
            // NOP!
            return;
        }
        if self.dimensions == dimensions && self.is_full_screen == is_full_screen {
            // It didn't really change
            return;
        }
        self.is_full_screen = is_full_screen;
        self.scaling_changed(dimensions, self.fonts.get_font_scale());
        self.emit_window_event("window-resized");
    }

    pub fn apply_scale_change(&mut self, dimensions: &Dimensions, font_scale: f64) {
        let config = &self.config;
        let font_size = config.font_size * font_scale;
        let theoretical_height = font_size * dimensions.dpi as f64 / 72.0;

        if theoretical_height < 2.0 {
            log::warn!(
                "refusing to go to an unreasonably small font scale {:?}
                       font_scale={} would yield font_height {}",
                dimensions,
                font_scale,
                theoretical_height
            );
            return;
        }

        let (prior_font, prior_dpi) = self.fonts.change_scaling(font_scale, dimensions.dpi);
        match RenderMetrics::new(&self.fonts) {
            Ok(metrics) => {
                self.render_metrics = metrics;
            }
            Err(err) => {
                log::error!(
                    "{:#} while attempting to scale font to {} with {:?}",
                    err,
                    font_scale,
                    dimensions
                );
                // Restore prior scaling factors
                self.fonts.change_scaling(prior_font, prior_dpi);
            }
        }
        if let Err(err) = self.recreate_texture_atlas(None) {
            log::error!("recreate_texture_atlas: {:#}", err);
        }
    }

    pub fn apply_dimensions(
        &mut self,
        dimensions: &Dimensions,
        scale_changed_cells: Option<RowsAndCols>,
    ) {
        self.dimensions = *dimensions;

        // Technically speaking, we should compute the rows and cols
        // from the new dimensions and apply those to the tabs, and
        // then for the scaling changed case, try to re-apply the
        // original rows and cols, but if we do that we end up
        // double resizing the tabs, so we speculatively apply the
        // final size, which in that case should result in a NOP
        // change to the tab size.

        let config = &self.config;

        let (size, dims) = if let Some(cell_dims) = scale_changed_cells {
            // Scaling preserves existing terminal dimensions, yielding a new
            // overall set of window dimensions
            let size = PtySize {
                rows: cell_dims.rows as u16,
                cols: cell_dims.cols as u16,
                pixel_height: cell_dims.rows as u16 * self.render_metrics.cell_size.height as u16,
                pixel_width: cell_dims.cols as u16 * self.render_metrics.cell_size.width as u16,
            };

            let rows = size.rows + if self.show_tab_bar { 1 } else { 0 };
            let cols = size.cols;

            let pixel_height = (rows * self.render_metrics.cell_size.height as u16)
                + (config.window_padding.top + config.window_padding.bottom);

            let pixel_width = (cols * self.render_metrics.cell_size.width as u16)
                + (config.window_padding.left + self.effective_right_padding(&config));

            let dims = Dimensions {
                pixel_width: pixel_width as usize,
                pixel_height: pixel_height as usize,
                dpi: dimensions.dpi,
            };

            (size, dims)
        } else {
            // Resize of the window dimensions may result in changed terminal dimensions
            let avail_width = dimensions.pixel_width.saturating_sub(
                (config.window_padding.left + self.effective_right_padding(&config)) as usize,
            );
            let avail_height = dimensions.pixel_height.saturating_sub(
                (config.window_padding.top + config.window_padding.bottom) as usize,
            );

            let rows = (avail_height / self.render_metrics.cell_size.height as usize)
                .saturating_sub(if self.show_tab_bar { 1 } else { 0 });
            let cols = avail_width / self.render_metrics.cell_size.width as usize;

            let size = PtySize {
                rows: rows as u16,
                cols: cols as u16,
                // Take care to use the exact pixel dimensions of the cells, rather
                // than the available space, so that apps that are sensitive to
                // the pixels-per-cell have consistent values at a given font size.
                // https://github.com/wez/wezterm/issues/535
                pixel_height: rows as u16 * self.render_metrics.cell_size.height as u16,
                pixel_width: cols as u16 * self.render_metrics.cell_size.width as u16,
            };

            (size, *dimensions)
        };

        if self.render_state.is_some() {
            self.terminal_size = size;
        }

        let mux = Mux::get().unwrap();
        if let Some(window) = mux.get_window(self.mux_window_id) {
            for tab in window.iter() {
                tab.resize(size);
            }
        };
        self.update_title();

        // Queue up a speculative resize in order to preserve the number of rows+cols
        if let Some(cell_dims) = scale_changed_cells {
            if let Some(window) = self.window.as_ref() {
                log::trace!("scale changed so resize to {:?} {:?}", cell_dims, dims);
                window.set_inner_size(dims.pixel_width, dims.pixel_height);
            }
        }
    }

    pub fn current_cell_dimensions(&self) -> RowsAndCols {
        RowsAndCols {
            rows: self.terminal_size.rows as usize,
            cols: self.terminal_size.cols as usize,
        }
    }

    #[allow(clippy::float_cmp)]
    pub fn scaling_changed(&mut self, dimensions: Dimensions, font_scale: f64) {
        let scale_changed =
            dimensions.dpi != self.dimensions.dpi || font_scale != self.fonts.get_font_scale();

        let scale_changed_cells = if scale_changed {
            let cell_dims = self.current_cell_dimensions();
            self.apply_scale_change(&dimensions, font_scale);
            Some(cell_dims)
        } else {
            None
        };

        self.apply_dimensions(&dimensions, scale_changed_cells);
    }

    /// Used for applying font size changes only; this takes into account
    /// the `adjust_window_size_when_changing_font_size` configuration and
    /// revises the scaling/resize change accordingly
    pub fn adjust_font_scale(&mut self, font_scale: f64) {
        if !self.is_full_screen && self.config.adjust_window_size_when_changing_font_size {
            self.scaling_changed(self.dimensions, font_scale);
        } else {
            let dimensions = self.dimensions;
            // Compute new font metrics
            self.apply_scale_change(&dimensions, font_scale);
            // Now revise the pty size to fit the window
            self.apply_dimensions(&dimensions, None);
        }
    }

    pub fn decrease_font_size(&mut self) {
        self.adjust_font_scale(self.fonts.get_font_scale() * 0.9);
    }

    pub fn increase_font_size(&mut self) {
        self.adjust_font_scale(self.fonts.get_font_scale() * 1.1);
    }

    pub fn reset_font_size(&mut self) {
        self.adjust_font_scale(1.0);
    }

    pub fn reset_font_and_window_size(&mut self) -> anyhow::Result<()> {
        let config = &self.config;
        let size = config.initial_size();
        let fontconfig = Rc::new(FontConfiguration::new(
            Some(config.clone()),
            config.dpi.unwrap_or_else(|| ::window::default_dpi()) as usize,
        )?);
        let render_metrics = RenderMetrics::new(&fontconfig)?;

        let terminal_size = PtySize {
            rows: size.rows as u16,
            cols: size.cols as u16,
            pixel_width: (render_metrics.cell_size.width as u16 * size.cols),
            pixel_height: (render_metrics.cell_size.height as u16 * size.rows),
        };

        let show_tab_bar = config.enable_tab_bar && !config.hide_tab_bar_if_only_one_tab;

        let rows_with_tab_bar = if show_tab_bar { 1 } else { 0 } + terminal_size.rows;
        let dimensions = Dimensions {
            pixel_width: ((terminal_size.cols * render_metrics.cell_size.width as u16)
                + config.window_padding.left
                + effective_right_padding(&config, &render_metrics))
                as usize,
            pixel_height: ((rows_with_tab_bar * render_metrics.cell_size.height as u16)
                + config.window_padding.top
                + config.window_padding.bottom) as usize,
            dpi: config.dpi.unwrap_or_else(|| ::window::default_dpi()) as usize,
        };

        self.apply_scale_change(&dimensions, 1.0);
        self.apply_dimensions(
            &dimensions,
            Some(RowsAndCols {
                rows: size.rows as usize,
                cols: size.cols as usize,
            }),
        );
        Ok(())
    }

    pub fn effective_right_padding(&self, config: &ConfigHandle) -> u16 {
        effective_right_padding(config, &self.render_metrics)
    }
}

/// Computes the effective padding for the RHS.
/// This is needed because the default is 0, but if the user has
/// enabled the scroll bar then they will expect it to have a reasonable
/// size unless they've specified differently.
pub fn effective_right_padding(config: &ConfigHandle, render_metrics: &RenderMetrics) -> u16 {
    if config.enable_scroll_bar && config.window_padding.right == 0 {
        render_metrics.cell_size.width as u16
    } else {
        config.window_padding.right as u16
    }
}
