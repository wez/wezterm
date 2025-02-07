use crate::quad::{QuadTrait, TripleLayerQuadAllocator, TripleLayerQuadAllocatorTrait};
use crate::termwindow::render::{
    resolve_fg_color_attr, same_hyperlink, update_next_frame_time, ClusterStyleCache,
    ComputeCellFgBgParams, ComputeCellFgBgResult, LineToElementParams, LineToElementShape,
    RenderScreenLineParams, RenderScreenLineResult,
};
use crate::termwindow::LineToElementShapeItem;
use ::window::DeadKeyStatus;
use anyhow::Context;
use config::{HsbTransform, TextStyle};
use std::ops::Range;
use std::rc::Rc;
use std::time::Instant;
use termwiz::cell::{unicode_column_width, Blink};
use termwiz::color::LinearRgba;
use termwiz::surface::CursorShape;
use wezterm_bidi::Direction;
use wezterm_term::color::ColorAttribute;
use wezterm_term::CellAttributes;

impl crate::TermWindow {
    /// "Render" a line of the terminal screen into the vertex buffer.
    /// This is nominally a matter of setting the fg/bg color and the
    /// texture coordinates for a given glyph.  There's a little bit
    /// of extra complexity to deal with multi-cell glyphs.
    pub fn render_screen_line(
        &self,
        params: RenderScreenLineParams,
        layers: &mut TripleLayerQuadAllocator,
    ) -> anyhow::Result<RenderScreenLineResult> {
        if params.line.is_double_height_bottom() {
            // The top and bottom lines are required to have the same content.
            // For the sake of simplicity, we render both of them as part of
            // rendering the top row, so we have nothing more to do here.
            return Ok(RenderScreenLineResult {
                invalidate_on_hover_change: false,
            });
        }

        let gl_state = self.render_state.as_ref().unwrap();

        let num_cols = params.dims.cols;

        let hsv = if params.is_active {
            None
        } else {
            Some(params.config.inactive_pane_hsb)
        };

        let width_scale = if !params.line.is_single_width() {
            2.0
        } else {
            1.0
        };

        let height_scale = if params.line.is_double_height_top() {
            2.0
        } else {
            1.0
        };

        let cell_width = params.render_metrics.cell_size.width as f32 * width_scale;
        let cell_height = params.render_metrics.cell_size.height as f32 * height_scale;
        let pos_y = (self.dimensions.pixel_height as f32 / -2.) + params.top_pixel_y;
        let gl_x = self.dimensions.pixel_width as f32 / -2.;

        let start = Instant::now();

        let cursor_idx = if params.pane.is_some()
            && params.is_active
            && params.stable_line_idx == Some(params.cursor.y)
        {
            Some(params.cursor.x)
        } else {
            None
        };

        // Referencing the text being composed, but only if it belongs to this pane
        let composing = if cursor_idx.is_some() {
            if let DeadKeyStatus::Composing(composing) = &self.dead_key_status {
                Some(composing)
            } else {
                None
            }
        } else {
            None
        };

        let mut composition_width = 0;

        let (_bidi_enabled, bidi_direction) = params.line.bidi_info();
        let direction = bidi_direction.direction();

        // Do we need to shape immediately, or can we use the pre-shaped data?
        if let Some(composing) = composing {
            composition_width = unicode_column_width(composing, None);
        }

        let cursor_cell = if params.stable_line_idx == Some(params.cursor.y) {
            params.line.get_cell(params.cursor.x)
        } else {
            None
        };

        let cursor_range = if composition_width > 0 {
            params.cursor.x..params.cursor.x + composition_width
        } else if params.stable_line_idx == Some(params.cursor.y) {
            params.cursor.x..params.cursor.x + cursor_cell.as_ref().map(|c| c.width()).unwrap_or(1)
        } else {
            0..0
        };

        let cursor_range_pixels = params.left_pixel_x + cursor_range.start as f32 * cell_width
            ..params.left_pixel_x + cursor_range.end as f32 * cell_width;

        let mut shaped = None;
        let mut invalidate_on_hover_change = false;

        if let Some(shape_key) = &params.shape_key {
            let mut cache = self.line_to_ele_shape_cache.borrow_mut();
            if let Some(entry) = cache.get(shape_key) {
                let expired = entry.expires.map(|i| Instant::now() >= i).unwrap_or(false);
                let hover_changed = if entry.invalidate_on_hover_change {
                    !same_hyperlink(
                        entry.current_highlight.as_ref(),
                        self.current_highlight.as_ref(),
                    )
                } else {
                    false
                };

                if !expired && !hover_changed {
                    self.update_next_frame_time(entry.expires);
                    shaped.replace(Rc::clone(&entry.shaped));
                }

                invalidate_on_hover_change = entry.invalidate_on_hover_change;
            }
        }

        let shaped = if let Some(shaped) = shaped {
            shaped
        } else {
            let params = LineToElementParams {
                config: params.config,
                line: params.line,
                palette: params.palette,
                window_is_transparent: params.window_is_transparent,
                reverse_video: params.dims.reverse_video,
                shape_key: &params.shape_key,
            };

            let (shaped, invalidate_on_hover) = self.build_line_element_shape(params)?;
            invalidate_on_hover_change = invalidate_on_hover;
            shaped
        };

        let bounding_rect = euclid::rect(
            params.left_pixel_x,
            params.top_pixel_y,
            params.pixel_width,
            cell_height,
        );

        fn phys(x: usize, num_cols: usize, direction: Direction) -> usize {
            match direction {
                Direction::LeftToRight => x,
                Direction::RightToLeft => num_cols - x,
            }
        }

        if params.dims.reverse_video {
            let mut quad = self
                .filled_rectangle(
                    layers,
                    0,
                    euclid::rect(
                        params.left_pixel_x,
                        params.top_pixel_y,
                        params.pixel_width,
                        cell_height,
                    ),
                    params.foreground,
                )
                .context("filled_rectangle")?;
            quad.set_hsv(hsv);
        }

        // Assume that we are drawing retro tab bar if there is no
        // stable_line_idx set.
        let is_tab_bar = params.stable_line_idx.is_none();

        // Make a pass to compute background colors.
        // Need to consider:
        // * background when it is not the default color
        // * Reverse video attribute
        for item in shaped.iter() {
            let cluster = &item.cluster;
            let attrs = &cluster.attrs;
            let cluster_width = cluster.width;

            let bg_is_default = attrs.background() == ColorAttribute::Default;
            let bg_color = params.palette.resolve_bg(attrs.background()).to_linear();

            let fg_color = resolve_fg_color_attr(
                &attrs,
                attrs.foreground(),
                &params.palette,
                &params.config,
                &Default::default(),
            );

            let (bg_color, bg_is_default) = {
                let mut fg = fg_color;
                let mut bg = bg_color;
                let mut bg_default = bg_is_default;

                // Check the line reverse_video flag and flip.
                if attrs.reverse() == !params.dims.reverse_video {
                    std::mem::swap(&mut fg, &mut bg);
                    bg_default = false;
                }

                (
                    bg.mul_alpha(self.config.text_background_opacity),
                    bg_default,
                )
            };

            if !bg_is_default {
                let x = params.left_pixel_x
                    + if params.use_pixel_positioning {
                        item.x_pos
                    } else {
                        phys(cluster.first_cell_idx, num_cols, direction) as f32 * cell_width
                    };

                let mut width = if params.use_pixel_positioning {
                    item.pixel_width
                } else {
                    cluster_width as f32 * cell_width
                };

                // If the tab bar is falling just short of the full width of the
                // window, extend it to fit.
                // <https://github.com/wezterm/wezterm/issues/2210>
                if is_tab_bar && (x + width + cell_width) > params.pixel_width {
                    width += cell_width;
                }

                let rect = euclid::rect(x, params.top_pixel_y, width, cell_height);
                if let Some(rect) = rect.intersection(&bounding_rect) {
                    let mut quad = self
                        .filled_rectangle(layers, 0, rect, bg_color)
                        .context("filled_rectangle")?;
                    quad.set_hsv(hsv);
                }
            }

            // Underlines
            if item.underline_tex_rect != params.white_space {
                // Draw one per cell, otherwise curly underlines
                // stretch across the whole span
                for i in 0..cluster_width {
                    let mut quad = layers.allocate(0).context("layers.allocate(0)")?;
                    let x = gl_x
                        + params.left_pixel_x
                        + if params.use_pixel_positioning {
                            item.x_pos
                        } else {
                            phys(cluster.first_cell_idx + i, num_cols, direction) as f32
                                * cell_width
                        };

                    quad.set_position(x, pos_y, x + cell_width, pos_y + cell_height);
                    quad.set_hsv(hsv);
                    quad.set_has_color(false);
                    quad.set_texture(item.underline_tex_rect);
                    quad.set_fg_color(item.underline_color);
                }
            }
        }

        // Render the selection background color.
        // This always uses a physical x position, regardles of the line
        // direction.
        let selection_pixel_range = if !params.selection.is_empty() {
            let start = params.left_pixel_x + (params.selection.start as f32 * cell_width);
            let width = (params.selection.end - params.selection.start) as f32 * cell_width;
            let mut quad = self
                .filled_rectangle(
                    layers,
                    0,
                    euclid::rect(start, params.top_pixel_y, width, cell_height),
                    params.selection_bg,
                )
                .context("filled_rectangle")?;

            quad.set_hsv(hsv);

            start..start + width
        } else {
            0.0..0.0
        };

        // Consider cursor
        if !cursor_range.is_empty() {
            let (fg_color, bg_color) = if let Some(c) = &cursor_cell {
                let attrs = c.attrs();

                let bg_color = params.palette.resolve_bg(attrs.background()).to_linear();

                let fg_color = resolve_fg_color_attr(
                    &attrs,
                    attrs.foreground(),
                    &params.palette,
                    &params.config,
                    &Default::default(),
                );

                (fg_color, bg_color)
            } else {
                (params.foreground, params.default_bg)
            };

            let ComputeCellFgBgResult {
                cursor_shape,
                cursor_border_color,
                cursor_border_color_alt,
                cursor_border_mix,
                ..
            } = self.compute_cell_fg_bg(ComputeCellFgBgParams {
                cursor: Some(params.cursor),
                selected: false,
                fg_color,
                bg_color,
                is_active_pane: params.is_active,
                config: params.config,
                selection_fg: params.selection_fg,
                selection_bg: params.selection_bg,
                cursor_fg: params.cursor_fg,
                cursor_bg: params.cursor_bg,
                cursor_is_default_color: params.cursor_is_default_color,
                cursor_border_color: params.cursor_border_color,
                pane: params.pane,
            });
            let pos_x = (self.dimensions.pixel_width as f32 / -2.)
                + params.left_pixel_x
                + (phys(params.cursor.x, num_cols, direction) as f32 * cell_width);

            if let Some(shape) = cursor_shape {
                let cursor_layer = match shape {
                    CursorShape::BlinkingBar | CursorShape::SteadyBar => 2,
                    _ => 0,
                };
                let mut quad = layers
                    .allocate(cursor_layer)
                    .with_context(|| format!("layers.allocate({cursor_layer})"))?;
                quad.set_hsv(hsv);
                quad.set_has_color(false);

                let mut draw_basic = true;

                if params.password_input {
                    let attrs = cursor_cell
                        .as_ref()
                        .map(|cell| cell.attrs().clone())
                        .unwrap_or_else(|| CellAttributes::blank());

                    let glyph = self
                        .resolve_lock_glyph(
                            &TextStyle::default(),
                            &attrs,
                            params.font.as_ref(),
                            gl_state,
                            &params.render_metrics,
                        )
                        .context("resolve_lock_glyph")?;

                    if let Some(sprite) = &glyph.texture {
                        let width = sprite.coords.size.width as f32 * glyph.scale as f32;
                        let height =
                            sprite.coords.size.height as f32 * glyph.scale as f32 * height_scale;

                        let pos_y = pos_y
                            + cell_height
                            + (params.render_metrics.descender.get() as f32
                                - (glyph.y_offset + glyph.bearing_y).get() as f32)
                                * height_scale;

                        let pos_x = pos_x + (glyph.x_offset + glyph.bearing_x).get() as f32;
                        quad.set_position(pos_x, pos_y, pos_x + width, pos_y + height);
                        quad.set_texture(sprite.texture_coords());
                        draw_basic = false;
                    }
                }

                if draw_basic {
                    quad.set_position(
                        pos_x,
                        pos_y,
                        pos_x + (cursor_range.end - cursor_range.start) as f32 * cell_width,
                        pos_y + cell_height,
                    );
                    quad.set_texture(
                        gl_state
                            .glyph_cache
                            .borrow_mut()
                            .cursor_sprite(
                                Some(shape),
                                &params.render_metrics,
                                (cursor_range.end - cursor_range.start) as u8,
                            )?
                            .texture_coords(),
                    );
                }

                quad.set_fg_color(cursor_border_color);
                quad.set_alt_color_and_mix_value(cursor_border_color_alt, cursor_border_mix);
            }
        }

        let mut overlay_images = vec![];

        // Number of cells we've rendered, starting from the edge of the line
        let mut visual_cell_idx = 0;

        let mut cluster_x_pos = match direction {
            Direction::LeftToRight => 0.,
            Direction::RightToLeft => params.pixel_width,
        };

        for item in shaped.iter() {
            let cluster = &item.cluster;
            let glyph_info = &item.glyph_info;
            let images = cluster.attrs.images().unwrap_or_else(|| vec![]);
            let valign_adjust = match cluster.attrs.vertical_align() {
                termwiz::cell::VerticalAlign::BaseLine => 0.,
                termwiz::cell::VerticalAlign::SuperScript => {
                    params.render_metrics.cell_size.height as f32 * -0.25
                }
                termwiz::cell::VerticalAlign::SubScript => {
                    params.render_metrics.cell_size.height as f32 * 0.25
                }
            };

            // TODO: remember logical/visual mapping for selection
            #[allow(unused_variables)]
            let mut phys_cell_idx = cluster.first_cell_idx;

            // Pre-decrement by the cluster width when doing RTL,
            // so that we can render it right-justified
            if direction == Direction::RightToLeft {
                cluster_x_pos -= if params.use_pixel_positioning {
                    item.pixel_width
                } else {
                    cluster.width as f32 * cell_width
                };
            }

            for info in glyph_info.iter() {
                let glyph = &info.glyph;

                if params.use_pixel_positioning
                    && params.left_pixel_x + cluster_x_pos + glyph.x_advance.get() as f32
                        >= params.left_pixel_x + params.pixel_width
                {
                    break;
                }

                for glyph_idx in 0..info.pos.num_cells as usize {
                    for img in &images {
                        if img.z_index() < 0 {
                            self.populate_image_quad(
                                &img,
                                gl_state,
                                layers,
                                0,
                                visual_cell_idx + glyph_idx,
                                &params,
                                hsv,
                                item.fg_color,
                            )?;
                        }
                    }
                }

                {
                    // First, resolve this glyph to a texture
                    let mut texture = glyph.texture.as_ref().cloned();

                    let mut top = cell_height
                        + (params.render_metrics.descender.get() as f32 + valign_adjust
                            - (glyph.y_offset + glyph.bearing_y).get() as f32)
                            * height_scale;

                    if self.config.custom_block_glyphs {
                        if let Some(block) = &info.block_key {
                            texture.replace(
                                gl_state
                                    .glyph_cache
                                    .borrow_mut()
                                    .cached_block(*block, &params.render_metrics)
                                    .context("cached_block")?,
                            );
                            // Custom glyphs don't have the same offsets as computed
                            // by the shaper, and are rendered relative to the cell
                            // top left, rather than the baseline.
                            top = 0.;
                        }
                    }

                    if let Some(texture) = texture {
                        // TODO: clipping, but we can do that based on pixels

                        let pos_x = cluster_x_pos
                            + if params.use_pixel_positioning {
                                (glyph.x_offset + glyph.bearing_x).get() as f32
                            } else {
                                0.
                            };

                        if pos_x > params.pixel_width {
                            log::trace!("breaking on overflow {} > {}", pos_x, params.pixel_width);
                            break;
                        }
                        let pos_x = pos_x + params.left_pixel_x;

                        // We need to conceptually slice this texture into
                        // up into strips that consider the cursor and selection
                        // background ranges. For ligatures that span cells, we'll
                        // need to explicitly render each strip independently so that
                        // we can set its foreground color to the appropriate color
                        // for the cursor/selection/regular background upon which
                        // it will be drawn.

                        /// Computes the intersection between r1 and r2.
                        /// It may be empty.
                        fn intersection(r1: &Range<f32>, r2: &Range<f32>) -> Range<f32> {
                            let start = r1.start.max(r2.start);
                            let end = r1.end.min(r2.end);
                            if end > start {
                                start..end
                            } else {
                                // Empty
                                start..start
                            }
                        }

                        /// Assess range `r` relative to `within`. If `r` intersects
                        /// `within` then return the 3 ranges that are subsets of `r`
                        /// which are to the left of `within`, intersecting `within`
                        /// and to the right of `within`.
                        /// If `r` and `within` do not intersect, returns `r` and
                        /// two empty ranges.
                        /// If `r` is itself an empty range, all returned ranges
                        /// will be empty.
                        fn range3(
                            r: &Range<f32>,
                            within: &Range<f32>,
                        ) -> (Range<f32>, Range<f32>, Range<f32>) {
                            if r.is_empty() {
                                return (r.clone(), r.clone(), r.clone());
                            }
                            let i = intersection(r, within);
                            if i.is_empty() {
                                return (r.clone(), i.clone(), i.clone());
                            }

                            let left = if i.start > r.start {
                                r.start..i.start
                            } else {
                                r.start..r.start
                            };

                            let right = if i.end < r.end {
                                i.end..r.end
                            } else {
                                r.end..r.end
                            };

                            (left, i, right)
                        }

                        let adjust = (glyph.x_offset + glyph.bearing_x).get() as f32;
                        let texture_range = pos_x + adjust
                            ..pos_x + adjust + (texture.coords.size.width as f32 * width_scale);

                        // First bucket the ranges according to cursor position
                        let (left, mid, right) = range3(&texture_range, &cursor_range_pixels);
                        // Then sub-divide the non-cursor ranges according to selection
                        let (la, lb, lc) = range3(&left, &selection_pixel_range);
                        let (ra, rb, rc) = range3(&right, &selection_pixel_range);

                        // and render each of these strips
                        for range in [la, lb, lc, mid, ra, rb, rc] {
                            if range.is_empty() {
                                continue;
                            }

                            let is_cursor = cursor_range_pixels.contains(&range.start);
                            let selected =
                                !is_cursor && selection_pixel_range.contains(&range.start);

                            let ComputeCellFgBgResult {
                                fg_color: glyph_color,
                                bg_color,
                                fg_color_alt,
                                fg_color_mix,
                                ..
                            } = self.compute_cell_fg_bg(ComputeCellFgBgParams {
                                cursor: if is_cursor { Some(params.cursor) } else { None },
                                selected,
                                fg_color: item.fg_color,
                                bg_color: item.bg_color,
                                is_active_pane: params.is_active,
                                config: params.config,
                                selection_fg: params.selection_fg,
                                selection_bg: params.selection_bg,
                                cursor_fg: params.cursor_fg,
                                cursor_bg: params.cursor_bg,
                                cursor_is_default_color: params.cursor_is_default_color,
                                cursor_border_color: params.cursor_border_color,
                                pane: params.pane,
                            });

                            if glyph_color == bg_color || cluster.attrs.invisible() {
                                // Essentially invisible: don't render it, as anti-aliasing
                                // can cause a ghostly outline of the invisible glyph to appear.
                                continue;
                            }

                            let pixel_rect = euclid::rect(
                                texture.coords.origin.x + (range.start - (pos_x + adjust)) as isize,
                                texture.coords.origin.y,
                                ((range.end - range.start) / width_scale) as isize,
                                texture.coords.size.height,
                            );

                            let texture_rect = texture.texture.to_texture_coords(pixel_rect);

                            let mut quad = layers.allocate(1).context("layers.allocate(1)")?;
                            quad.set_position(
                                gl_x + range.start,
                                pos_y + top,
                                gl_x + range.end,
                                pos_y + top + texture.coords.size.height as f32 * height_scale,
                            );
                            quad.set_fg_color(glyph_color);
                            quad.set_alt_color_and_mix_value(fg_color_alt, fg_color_mix);
                            quad.set_texture(texture_rect);
                            quad.set_hsv(if glyph.brightness_adjust != 1.0 {
                                let hsv = hsv.unwrap_or_else(|| HsbTransform::default());
                                Some(HsbTransform {
                                    brightness: hsv.brightness * glyph.brightness_adjust,
                                    ..hsv
                                })
                            } else {
                                hsv
                            });
                            quad.set_has_color(glyph.has_color);
                        }
                    }
                }

                for glyph_idx in 0..info.pos.num_cells as usize {
                    for img in &images {
                        if img.z_index() >= 0 {
                            overlay_images.push((
                                visual_cell_idx + glyph_idx,
                                img.clone(),
                                item.fg_color,
                            ));
                        }
                    }
                }
                phys_cell_idx += info.pos.num_cells as usize;
                visual_cell_idx += info.pos.num_cells as usize;
                cluster_x_pos += if params.use_pixel_positioning {
                    glyph.x_advance.get() as f32 * width_scale
                } else {
                    info.pos.num_cells as f32 * cell_width
                };
            }

            match direction {
                Direction::RightToLeft => {
                    // And decrement it again
                    cluster_x_pos -= if params.use_pixel_positioning {
                        item.pixel_width * width_scale
                    } else {
                        cluster.width as f32 * cell_width
                    };
                }
                Direction::LeftToRight => {}
            }
        }

        for (cell_idx, img, glyph_color) in overlay_images {
            self.populate_image_quad(
                &img,
                gl_state,
                layers,
                2,
                phys(cell_idx, num_cols, direction),
                &params,
                hsv,
                glyph_color,
            )
            .context("populate_image_quad")?;
        }

        metrics::histogram!("render_screen_line").record(start.elapsed());

        Ok(RenderScreenLineResult {
            invalidate_on_hover_change,
        })
    }

    fn build_line_element_shape(
        &self,
        params: LineToElementParams,
    ) -> anyhow::Result<(Rc<Vec<LineToElementShape>>, bool)> {
        let (bidi_enabled, bidi_direction) = params.line.bidi_info();
        let bidi_hint = if bidi_enabled {
            Some(bidi_direction)
        } else {
            None
        };
        let cell_clusters = if let Some((cursor_x, composing)) =
            params.shape_key.as_ref().and_then(|k| k.composing.as_ref())
        {
            // Create an updated line with the composition overlaid
            let mut line = params.line.clone();
            let seqno = line.current_seqno();
            line.overlay_text_with_attribute(*cursor_x, &composing, CellAttributes::blank(), seqno);
            line.cluster(bidi_hint)
        } else {
            params.line.cluster(bidi_hint)
        };

        let gl_state = self.render_state.as_ref().unwrap();
        let mut shaped = vec![];
        let mut last_style = None;
        let mut x_pos = 0.;
        let mut expires = None;
        let mut invalidate_on_hover_change = false;

        for cluster in &cell_clusters {
            if !matches!(last_style.as_ref(), Some(ClusterStyleCache{attrs,..}) if *attrs == &cluster.attrs)
            {
                let attrs = &cluster.attrs;
                let style = self.fonts.match_style(params.config, attrs);
                let hyperlink = attrs.hyperlink();
                let is_highlited_hyperlink =
                    same_hyperlink(hyperlink, self.current_highlight.as_ref());
                if hyperlink.is_some() {
                    invalidate_on_hover_change = true;
                }
                // underline and strikethrough
                let underline_tex_rect = gl_state
                    .glyph_cache
                    .borrow_mut()
                    .cached_line_sprite(
                        is_highlited_hyperlink,
                        attrs.strikethrough(),
                        attrs.underline(),
                        attrs.overline(),
                        &self.render_metrics,
                    )?
                    .texture_coords();
                let bg_is_default = attrs.background() == ColorAttribute::Default;
                let bg_color = params.palette.resolve_bg(attrs.background()).to_linear();

                let fg_color = resolve_fg_color_attr(
                    &attrs,
                    attrs.foreground(),
                    &params.palette,
                    &params.config,
                    style,
                );
                let (fg_color, bg_color, bg_is_default) = {
                    let mut fg = fg_color;
                    let mut bg = bg_color;
                    let mut bg_default = bg_is_default;

                    // Check the line reverse_video flag and flip.
                    if attrs.reverse() == !params.reverse_video {
                        std::mem::swap(&mut fg, &mut bg);
                        bg_default = false;
                    }

                    // Check for blink, and if this is the "not-visible"
                    // part of blinking then set fg = bg.  This is a cheap
                    // means of getting it done without impacting other
                    // features.
                    let blink_rate = match attrs.blink() {
                        Blink::None => None,
                        Blink::Slow => {
                            Some((params.config.text_blink_rate, self.blink_state.borrow_mut()))
                        }
                        Blink::Rapid => Some((
                            params.config.text_blink_rate_rapid,
                            self.rapid_blink_state.borrow_mut(),
                        )),
                    };
                    if let Some((blink_rate, mut colorease)) = blink_rate {
                        if blink_rate != 0 {
                            let (intensity, next) = colorease.intensity_continuous();

                            let (r1, g1, b1, a) = bg.tuple();
                            let (r, g, b, _a) = fg.tuple();
                            fg = LinearRgba::with_components(
                                r1 + (r - r1) * intensity,
                                g1 + (g - g1) * intensity,
                                b1 + (b - b1) * intensity,
                                a,
                            );

                            update_next_frame_time(&mut expires, Some(next));
                            self.update_next_frame_time(Some(next));
                        }
                    }

                    (fg, bg, bg_default)
                };

                let glyph_color = fg_color;
                let underline_color = match attrs.underline_color() {
                    ColorAttribute::Default => fg_color,
                    c => resolve_fg_color_attr(&attrs, c, &params.palette, &params.config, style),
                };

                let (bg_r, bg_g, bg_b, _) = bg_color.tuple();
                let bg_color = LinearRgba::with_components(
                    bg_r,
                    bg_g,
                    bg_b,
                    if params.window_is_transparent && bg_is_default {
                        0.0
                    } else {
                        params.config.text_background_opacity
                    },
                );

                last_style.replace(ClusterStyleCache {
                    attrs,
                    style,
                    underline_tex_rect: underline_tex_rect.clone(),
                    bg_color,
                    fg_color: glyph_color,
                    underline_color,
                });
            }

            let style_params = last_style.as_ref().expect("we just set it up").clone();

            let glyph_info = self.cached_cluster_shape(
                style_params.style,
                &cluster,
                &gl_state,
                None,
                &self.render_metrics,
            )?;
            let pixel_width = glyph_info
                .iter()
                .map(|info| info.glyph.x_advance.get() as f32)
                .sum();

            shaped.push(LineToElementShape {
                underline_tex_rect: style_params.underline_tex_rect,
                bg_color: style_params.bg_color,
                fg_color: style_params.fg_color,
                underline_color: style_params.underline_color,
                pixel_width,
                cluster: cluster.clone(),
                glyph_info,
                x_pos,
            });

            x_pos += pixel_width;
        }

        let shaped = Rc::new(shaped);

        if let Some(shape_key) = params.shape_key {
            self.line_to_ele_shape_cache.borrow_mut().put(
                shape_key.clone(),
                LineToElementShapeItem {
                    expires,
                    shaped: Rc::clone(&shaped),
                    invalidate_on_hover_change,
                    current_highlight: if invalidate_on_hover_change {
                        self.current_highlight.clone()
                    } else {
                        None
                    },
                },
            );
        }

        Ok((shaped, invalidate_on_hover_change))
    }
}
