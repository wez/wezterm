use crate::utilsprites::RenderMetrics;
use ::window::{Dimensions, Window, WindowOps, WindowState};
use config::{ConfigHandle, DimensionContext};
use mux::Mux;
use std::rc::Rc;
use wezterm_font::FontConfiguration;
use wezterm_term::TerminalSize;

#[derive(Debug, Clone, Copy)]
pub struct RowsAndCols {
    pub rows: usize,
    pub cols: usize,
}

impl super::TermWindow {
    pub fn resize(
        &mut self,
        dimensions: Dimensions,
        window_state: WindowState,
        window: &Window,
        live_resizing: bool,
    ) {
        log::trace!(
            "resize event, live={} current cells: {:?}, current dims: {:?}, new dims: {:?} window_state:{:?}",
            live_resizing,
            self.current_cell_dimensions(),
            self.dimensions,
            dimensions,
            window_state,
        );
        if dimensions.pixel_width == 0 || dimensions.pixel_height == 0 {
            // on windows, this can happen when minimizing the window.
            // NOP!
            log::trace!("new dimensions are zero: NOP!");
            return;
        }
        if self.dimensions == dimensions && self.window_state == window_state {
            // It didn't really change
            log::trace!("dimensions didn't change NOP!");
            return;
        }
        let last_state = self.window_state;
        self.window_state = window_state;
        self.quad_generation += 1;
        if last_state != self.window_state {
            self.load_os_parameters();
        }

        if let Some(webgpu) = self.webgpu.as_mut() {
            webgpu.resize(dimensions);
        }

        // For simple, user-interactive resizes where the dpi doesn't change,
        // skip our scaling recalculation
        if live_resizing && self.dimensions.dpi == dimensions.dpi {
            self.apply_dimensions(&dimensions, None, window);
        } else {
            self.scaling_changed(dimensions, self.fonts.get_font_scale(), window);
        }
        if let Some(modal) = self.get_modal() {
            modal.reconfigure(self);
        }
        self.emit_window_event("window-resized", None);
    }

    pub fn apply_scale_change(
        &mut self,
        dimensions: &Dimensions,
        font_scale: f64,
        window: &Window,
    ) {
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

        window.set_resize_increments(
            if self.config.use_resize_increments {
                self.render_metrics.cell_size.width as u16
            } else {
                1
            },
            if self.config.use_resize_increments {
                self.render_metrics.cell_size.height as u16
            } else {
                1
            },
        );

        if let Err(err) = self.recreate_texture_atlas(None) {
            log::error!("recreate_texture_atlas: {:#}", err);
        }
        self.invalidate_fancy_tab_bar();
        self.invalidate_modal();
    }

    pub fn apply_dimensions(
        &mut self,
        dimensions: &Dimensions,
        mut scale_changed_cells: Option<RowsAndCols>,
        window: &Window,
    ) {
        log::trace!(
            "apply_dimensions {:?} scale_changed_cells {:?}. window_state {:?}",
            dimensions,
            scale_changed_cells,
            self.window_state
        );
        let saved_dims = self.dimensions;
        self.dimensions = *dimensions;
        self.quad_generation += 1;

        if scale_changed_cells.is_some() && !self.window_state.can_resize() {
            log::warn!(
                "cannot resize window to match {:?} because window_state is {:?}",
                scale_changed_cells,
                self.window_state
            );
            scale_changed_cells.take();
        }

        // Technically speaking, we should compute the rows and cols
        // from the new dimensions and apply those to the tabs, and
        // then for the scaling changed case, try to re-apply the
        // original rows and cols, but if we do that we end up
        // double resizing the tabs, so we speculatively apply the
        // final size, which in that case should result in a NOP
        // change to the tab size.

        let config = &self.config;

        let tab_bar_height = if self.show_tab_bar {
            self.tab_bar_pixel_height().unwrap_or(0.)
        } else {
            0.
        };

        let border = self.get_os_border();

        let (size, dims) = if let Some(cell_dims) = scale_changed_cells {
            // Scaling preserves existing terminal dimensions, yielding a new
            // overall set of window dimensions
            let size = TerminalSize {
                rows: cell_dims.rows,
                cols: cell_dims.cols,
                pixel_height: cell_dims.rows * self.render_metrics.cell_size.height as usize,
                pixel_width: cell_dims.cols * self.render_metrics.cell_size.width as usize,
                dpi: dimensions.dpi as u32,
            };

            let rows = size.rows;
            let cols = size.cols;

            let h_context = DimensionContext {
                dpi: dimensions.dpi as f32,
                pixel_max: size.pixel_width as f32,
                pixel_cell: self.render_metrics.cell_size.width as f32,
            };
            let v_context = DimensionContext {
                dpi: dimensions.dpi as f32,
                pixel_max: size.pixel_height as f32,
                pixel_cell: self.render_metrics.cell_size.height as f32,
            };
            let padding_left = config.window_padding.left.evaluate_as_pixels(h_context) as usize;
            let padding_top = config.window_padding.top.evaluate_as_pixels(v_context) as usize;
            let padding_bottom =
                config.window_padding.bottom.evaluate_as_pixels(v_context) as usize;
            let padding_right = effective_right_padding(&config, h_context);

            let pixel_height = (rows * self.render_metrics.cell_size.height as usize)
                + (padding_top + padding_bottom)
                + tab_bar_height as usize;

            let pixel_width = (cols * self.render_metrics.cell_size.width as usize)
                + (padding_left + padding_right);

            let dims = Dimensions {
                pixel_width: pixel_width as usize,
                pixel_height: pixel_height as usize,
                dpi: dimensions.dpi,
            };

            (size, dims)
        } else {
            // Resize of the window dimensions may result in changed terminal dimensions

            let h_context = DimensionContext {
                dpi: dimensions.dpi as f32,
                pixel_max: self.terminal_size.pixel_width as f32,
                pixel_cell: self.render_metrics.cell_size.width as f32,
            };
            let v_context = DimensionContext {
                dpi: dimensions.dpi as f32,
                pixel_max: self.terminal_size.pixel_height as f32,
                pixel_cell: self.render_metrics.cell_size.height as f32,
            };
            let padding_left = config.window_padding.left.evaluate_as_pixels(h_context) as usize;
            let padding_top = config.window_padding.top.evaluate_as_pixels(v_context) as usize;
            let padding_bottom =
                config.window_padding.bottom.evaluate_as_pixels(v_context) as usize;
            let padding_right = effective_right_padding(&config, h_context);

            let avail_width = dimensions.pixel_width.saturating_sub(
                (padding_left + padding_right) as usize
                    + (border.left + border.right).get() as usize,
            );
            let avail_height = dimensions
                .pixel_height
                .saturating_sub(
                    (padding_top + padding_bottom) as usize
                        + (border.top + border.bottom).get() as usize,
                )
                .saturating_sub(tab_bar_height as usize);

            let rows = avail_height / self.render_metrics.cell_size.height as usize;
            let cols = avail_width / self.render_metrics.cell_size.width as usize;

            let size = TerminalSize {
                rows,
                cols,
                // Take care to use the exact pixel dimensions of the cells, rather
                // than the available space, so that apps that are sensitive to
                // the pixels-per-cell have consistent values at a given font size.
                // https://github.com/wez/wezterm/issues/535
                pixel_height: rows * self.render_metrics.cell_size.height as usize,
                pixel_width: cols * self.render_metrics.cell_size.width as usize,
                dpi: dimensions.dpi as u32,
            };

            (size, *dimensions)
        };

        log::trace!("apply_dimensions computed size {:?}, dims {:?}", size, dims);

        self.terminal_size = size;

        let mux = Mux::get().unwrap();
        if let Some(window) = mux.get_window(self.mux_window_id) {
            for tab in window.iter() {
                tab.resize(size);
            }
        };
        self.resize_overlays();
        self.invalidate_fancy_tab_bar();
        self.update_title();

        // Queue up a speculative resize in order to preserve the number of rows+cols
        if let Some(cell_dims) = scale_changed_cells {
            // If we don't think the dimensions have changed, don't request
            // the window to change.  This seems to help on Wayland where
            // we won't know what size the compositor thinks we should have
            // when we're first opened, until after it sends us a configure event.
            // If we send this too early, it will trump that configure event
            // and we'll end up with weirdness where our window renders in the
            // middle of a larger region that the compositor thinks we live in.
            // Wayland is weird!
            if saved_dims != dims {
                log::trace!(
                    "scale changed so resize from {:?} to {:?} {:?} (event called with {:?})",
                    saved_dims,
                    dims,
                    cell_dims,
                    dimensions
                );
                // Stash this size pre-emptively. Without this, on Windows,
                // when the font scaling is changed we can end up not seeing
                // these dimensions and the scaling_changed logic ends up
                // comparing two dimensions that have the same DPI and recomputing
                // an adjusted terminal size.
                // eg: rather than a simple old-dpi -> new dpi transition, we'd
                // see old-dpi -> new dpi, call set_inner_size, then see a
                // new-dpi -> new-dpi adjustment with a slightly different
                // pixel geometry which is considered to be a user-driven resize.
                // Stashing the dimensions here avoids that misconception.
                self.dimensions = dims;
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
    pub fn scaling_changed(&mut self, dimensions: Dimensions, font_scale: f64, window: &Window) {
        fn dpi_adjusted(n: usize, dpi: usize) -> f32 {
            n as f32 / dpi as f32
        }

        /// On Windows, scaling changes may adjust the pixel geometry by a few pixels,
        /// so this function checks if we're in a close-enough ballpark.
        fn close_enough(a: f32, b: f32) -> bool {
            let diff = (a - b).abs();
            diff < 10.
        }

        // Distinguish between eg: dpi being detected as double the initial dpi (where
        // the pixel dimensions don't change), and the dpi change being detected, but
        // where the window manager also decides to tile/resize the window.
        // In the latter case, we don't want to preserve the terminal rows/cols.
        let simple_dpi_change = dimensions.dpi != self.dimensions.dpi
            && ((close_enough(
                dpi_adjusted(dimensions.pixel_height, dimensions.dpi),
                dpi_adjusted(self.dimensions.pixel_height, self.dimensions.dpi),
            ) && close_enough(
                dpi_adjusted(dimensions.pixel_width, dimensions.dpi),
                dpi_adjusted(self.dimensions.pixel_width, self.dimensions.dpi),
            )) || (close_enough(
                dimensions.pixel_width as f32,
                self.dimensions.pixel_width as f32,
            ) && close_enough(
                dimensions.pixel_height as f32,
                self.dimensions.pixel_height as f32,
            )));

        let dpi_changed = dimensions.dpi != self.dimensions.dpi;
        let font_scale_changed = font_scale != self.fonts.get_font_scale();
        let scale_changed = dpi_changed || font_scale_changed;

        log::trace!(
            "dpi_changed={}, font_scale_changed={} scale_changed={} simple_dpi_change={}",
            dpi_changed,
            font_scale_changed,
            scale_changed,
            simple_dpi_change
        );

        let cell_dims = self.current_cell_dimensions();

        if scale_changed {
            self.apply_scale_change(&dimensions, font_scale, window);
        }

        let scale_changed_cells = if font_scale_changed || simple_dpi_change {
            Some(cell_dims)
        } else {
            None
        };

        log::trace!(
            "scaling_changed, follow with applying dimensions. scale_changed_cells={:?}",
            scale_changed_cells
        );
        self.apply_dimensions(&dimensions, scale_changed_cells, window);
    }

    /// Used for applying font size changes only; this takes into account
    /// the `adjust_window_size_when_changing_font_size` configuration and
    /// revises the scaling/resize change accordingly
    pub fn adjust_font_scale(&mut self, font_scale: f64, window: &Window) {
        if self.window_state.can_resize() && self.config.adjust_window_size_when_changing_font_size
        {
            self.scaling_changed(self.dimensions, font_scale, window);
        } else {
            let dimensions = self.dimensions;
            // Compute new font metrics
            self.apply_scale_change(&dimensions, font_scale, window);
            // Now revise the pty size to fit the window
            self.apply_dimensions(&dimensions, None, window);
        }
    }

    pub fn decrease_font_size(&mut self, window: &Window) {
        self.adjust_font_scale(self.fonts.get_font_scale() / 1.1, window);
    }

    pub fn increase_font_size(&mut self, window: &Window) {
        self.adjust_font_scale(self.fonts.get_font_scale() * 1.1, window);
    }

    pub fn reset_font_size(&mut self, window: &Window) {
        self.adjust_font_scale(1.0, window);
    }

    pub fn set_window_size(&mut self, size: TerminalSize, window: &Window) -> anyhow::Result<()> {
        let config = &self.config;
        let fontconfig = Rc::new(FontConfiguration::new(
            Some(config.clone()),
            self.dimensions.dpi,
        )?);
        let render_metrics = RenderMetrics::new(&fontconfig)?;

        let terminal_size = TerminalSize {
            rows: size.rows,
            cols: size.cols,
            pixel_width: (render_metrics.cell_size.width as usize * size.cols),
            pixel_height: (render_metrics.cell_size.height as usize * size.rows),
            dpi: size.dpi,
        };

        let show_tab_bar = config.enable_tab_bar && !config.hide_tab_bar_if_only_one_tab;
        let tab_bar_height = if show_tab_bar {
            self.tab_bar_pixel_height()? as usize
        } else {
            0
        };

        let h_context = DimensionContext {
            dpi: self.dimensions.dpi as f32,
            pixel_max: self.dimensions.pixel_width as f32,
            pixel_cell: render_metrics.cell_size.width as f32,
        };
        let v_context = DimensionContext {
            dpi: self.dimensions.dpi as f32,
            pixel_max: self.dimensions.pixel_height as f32,
            pixel_cell: render_metrics.cell_size.height as f32,
        };
        let padding_left = config.window_padding.left.evaluate_as_pixels(h_context) as usize;
        let padding_top = config.window_padding.top.evaluate_as_pixels(v_context) as usize;
        let padding_bottom = config.window_padding.bottom.evaluate_as_pixels(v_context) as usize;

        let dimensions = Dimensions {
            pixel_width: ((terminal_size.cols as usize * render_metrics.cell_size.width as usize)
                + padding_left
                + effective_right_padding(&config, h_context)),
            pixel_height: ((terminal_size.rows as usize * render_metrics.cell_size.height as usize)
                + padding_top
                + padding_bottom) as usize
                + tab_bar_height,
            dpi: self.dimensions.dpi,
        };

        self.apply_scale_change(&dimensions, 1.0, window);
        self.apply_dimensions(
            &dimensions,
            Some(RowsAndCols {
                rows: size.rows as usize,
                cols: size.cols as usize,
            }),
            window,
        );
        Ok(())
    }

    pub fn reset_font_and_window_size(&mut self, window: &Window) -> anyhow::Result<()> {
        let size = self.config.initial_size(self.dimensions.dpi as u32);
        self.set_window_size(size, window)
    }

    pub fn effective_right_padding(&self, config: &ConfigHandle) -> usize {
        effective_right_padding(
            config,
            DimensionContext {
                pixel_cell: self.render_metrics.cell_size.width as f32,
                dpi: self.dimensions.dpi as f32,
                pixel_max: self.dimensions.pixel_width as f32,
            },
        )
    }
}

/// Computes the effective padding for the RHS.
/// This is needed because the default is 0, but if the user has
/// enabled the scroll bar then they will expect it to have a reasonable
/// size unless they've specified differently.
pub fn effective_right_padding(config: &ConfigHandle, context: DimensionContext) -> usize {
    if config.enable_scroll_bar && config.window_padding.right.is_zero() {
        context.pixel_cell as usize
    } else {
        config.window_padding.right.evaluate_as_pixels(context) as usize
    }
}
