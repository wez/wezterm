use crate::glium::texture::SrgbTexture2d;
use crate::glyphcache::{BlockKey, CachedGlyph, GlyphCache};
use crate::shapecache::*;
use crate::termwindow::{BorrowedShapeCacheKey, MappedQuads, RenderState, ScrollHit, ShapedInfo};
use ::window::bitmaps::atlas::OutOfTextureSpace;
use ::window::bitmaps::{TextureCoord, TextureRect, TextureSize};
use ::window::glium;
use ::window::glium::uniforms::{
    MagnifySamplerFilter, MinifySamplerFilter, Sampler, SamplerWrapFunction,
};
use ::window::glium::{uniform, BlendingFunction, LinearBlendingFactor, Surface};
use ::window::WindowOps;
use anyhow::anyhow;
use config::ConfigHandle;
use config::TextStyle;
use mux::pane::Pane;
use mux::renderable::{RenderableDimensions, StableCursorPosition};
use mux::tab::{PositionedPane, PositionedSplit, SplitDirection};
use std::ops::Range;
use std::rc::Rc;
use std::time::Instant;
use termwiz::cellcluster::CellCluster;
use termwiz::surface::{CursorShape, CursorVisibility};
use wezterm_font::units::PixelLength;
use wezterm_font::{ClearShapeCache, GlyphInfo};
use wezterm_term::color::{ColorAttribute, ColorPalette, RgbColor};
use wezterm_term::{CellAttributes, Line, StableRowIndex};
use window::bitmaps::atlas::SpriteSlice;
use window::bitmaps::Texture2d;
use window::color::LinearRgba;

pub struct RenderScreenLineOpenGLParams<'a> {
    pub line_idx: usize,
    pub stable_line_idx: Option<StableRowIndex>,
    pub line: &'a Line,
    pub selection: Range<usize>,
    pub cursor: &'a StableCursorPosition,
    pub palette: &'a ColorPalette,
    pub dims: &'a RenderableDimensions,
    pub config: &'a ConfigHandle,
    pub pos: &'a PositionedPane,

    pub cursor_border_color: LinearRgba,
    pub foreground: LinearRgba,
    pub is_active: bool,

    pub selection_fg: LinearRgba,
    pub selection_bg: LinearRgba,
    pub cursor_fg: LinearRgba,
    pub cursor_bg: LinearRgba,
}

pub struct ComputeCellFgBgParams<'a> {
    pub stable_line_idx: Option<StableRowIndex>,
    pub cell_idx: usize,
    pub cursor: &'a StableCursorPosition,
    pub selection: &'a Range<usize>,
    pub fg_color: LinearRgba,
    pub bg_color: LinearRgba,
    pub palette: &'a ColorPalette,
    pub is_active_pane: bool,
    pub config: &'a ConfigHandle,
    pub selection_fg: LinearRgba,
    pub selection_bg: LinearRgba,
    pub cursor_fg: LinearRgba,
    pub cursor_bg: LinearRgba,
}

pub struct ComputeCellFgBgResult {
    pub fg_color: LinearRgba,
    pub bg_color: LinearRgba,
    pub cursor_shape: Option<CursorShape>,
}

impl super::TermWindow {
    pub fn paint_impl(&mut self, frame: &mut glium::Frame) {
        // If nothing on screen needs animating, then we can avoid
        // invalidating as frequently
        *self.has_animation.borrow_mut() = None;

        self.check_for_config_reload();
        let start = Instant::now();

        {
            let background_alpha = (self.config.window_background_opacity * 255.0) as u8;
            let palette = self.palette();
            let background = rgbcolor_alpha_to_window_color(palette.background, background_alpha);

            let (r, g, b, a) = background.tuple();
            frame.clear_color(r, g, b, a);
        }

        for pass in 0.. {
            match self.paint_opengl_pass() {
                Ok(_) => break,
                Err(err) => {
                    if let Some(&OutOfTextureSpace {
                        size: Some(size),
                        current_size,
                    }) = err.root_cause().downcast_ref::<OutOfTextureSpace>()
                    {
                        let result = if pass == 0 {
                            // Let's try clearing out the atlas and trying again
                            // self.clear_texture_atlas()
                            log::trace!("recreate_texture_atlas");
                            self.recreate_texture_atlas(Some(current_size))
                        } else {
                            log::trace!("grow texture atlas to {}", size);
                            self.recreate_texture_atlas(Some(size))
                        };

                        if let Err(err) = result {
                            log::error!(
                                "Failed to {} texture: {}",
                                if pass == 0 { "clear" } else { "resize" },
                                err
                            );
                            break;
                        }
                    } else if err.root_cause().downcast_ref::<ClearShapeCache>().is_some() {
                        self.shape_cache.borrow_mut().clear();
                    } else {
                        log::error!("paint_opengl_pass failed: {:#}", err);
                        break;
                    }
                }
            }
        }
        log::debug!("paint_impl before call_draw elapsed={:?}", start.elapsed());

        self.call_draw(frame).ok();
        log::debug!("paint_impl elapsed={:?}", start.elapsed());
        metrics::histogram!("gui.paint.opengl", start.elapsed());
        self.update_title_post_status();
    }

    fn update_next_frame_time(&self, next_due: Option<Instant>) {
        if let Some(next_due) = next_due {
            let mut has_anim = self.has_animation.borrow_mut();
            match *has_anim {
                None => {
                    has_anim.replace(next_due);
                }
                Some(t) if next_due < t => {
                    has_anim.replace(next_due);
                }
                _ => {}
            }
        }
    }

    pub fn paint_pane_opengl(&mut self, pos: &PositionedPane) -> anyhow::Result<()> {
        // We typically check this periodically in the background as part
        // of deciding whether to repaint, but there are some situations
        // where the model may have been marked dirty and we re-render
        // very quickly.  If we don't check for dirty lines here before
        // we call get_lines_with_hyperlinks_applied below, then we'll
        // not clear the selection when we should.
        self.check_for_dirty_lines_and_invalidate_selection(pos);

        let config = &self.config;
        let palette = pos.pane.palette();

        let background_color = palette.resolve_bg(wezterm_term::color::ColorAttribute::Default);
        let first_line_offset = if self.show_tab_bar { 1 } else { 0 };

        let cursor = pos.pane.get_cursor_position();
        if pos.is_active {
            self.prev_cursor.update(&cursor);
        }

        let current_viewport = self.get_viewport(pos.pane.pane_id());
        let (stable_top, lines);
        let dims = pos.pane.get_dimensions();

        {
            let stable_range = match current_viewport {
                Some(top) => top..top + dims.viewport_rows as StableRowIndex,
                None => dims.physical_top..dims.physical_top + dims.viewport_rows as StableRowIndex,
            };

            let (top, vp_lines) = pos
                .pane
                .get_lines_with_hyperlinks_applied(stable_range, &self.config.hyperlink_rules);
            stable_top = top;
            lines = vp_lines;
        }

        let gl_state = self.render_state.as_ref().unwrap();
        let mut vb = gl_state.glyph_vertex_buffer.borrow_mut();

        let start = Instant::now();
        let mut quads = gl_state.quads.map(&mut vb);
        log::trace!("quad map elapsed {:?}", start.elapsed());

        let cursor_border_color =
            rgbcolor_to_window_color(if self.config.force_reverse_video_cursor {
                palette.foreground
            } else {
                palette.cursor_border
            });
        let foreground = rgbcolor_to_window_color(palette.foreground);

        if self.show_tab_bar && pos.index == 0 {
            let tab_dims = RenderableDimensions {
                cols: self.terminal_size.cols as _,
                ..dims
            };
            self.render_screen_line_opengl(
                RenderScreenLineOpenGLParams {
                    line_idx: 0,
                    stable_line_idx: None,
                    line: self.tab_bar.line(),
                    selection: 0..0,
                    cursor: &cursor,
                    palette: &palette,
                    dims: &tab_dims,
                    config: &config,
                    cursor_border_color,
                    foreground,
                    pos,
                    is_active: true,
                    selection_fg: LinearRgba::default(),
                    selection_bg: LinearRgba::default(),
                    cursor_fg: LinearRgba::default(),
                    cursor_bg: LinearRgba::default(),
                },
                &mut quads,
            )?;
        }

        // TODO: we only have a single scrollbar in a single position.
        // We only update it for the active pane, but we should probably
        // do a per-pane scrollbar.  That will require more extensive
        // changes to ScrollHit, mouse positioning, PositionedPane
        // and tab size calculation.
        if pos.is_active {
            let (thumb_top, thumb_size, color) = if self.show_scroll_bar {
                let info = ScrollHit::thumb(
                    &*pos.pane,
                    current_viewport,
                    self.terminal_size,
                    &self.dimensions,
                );
                let thumb_top = info.top as f32;
                let thumb_size = info.height as f32;
                let color = rgbcolor_to_window_color(palette.scrollbar_thumb);
                (thumb_top, thumb_size, color)
            } else {
                let color = rgbcolor_to_window_color(background_color);
                (0., 0., color)
            };

            let mut quad = quads.scroll_thumb();

            // Adjust the scrollbar thumb position
            let top = (self.dimensions.pixel_height as f32 / -2.0) + thumb_top;
            let bottom = top + thumb_size;

            let config = &self.config;
            let padding = self.effective_right_padding(&config) as f32;

            let right = self.dimensions.pixel_width as f32 / 2.;
            let left = right - padding;

            let white_space = gl_state.util_sprites.white_space.texture_coords();

            quad.set_bg_color(color);
            quad.set_fg_color(color);
            quad.set_underline_color(color);
            quad.set_position(left, top, right, bottom);
            quad.set_texture(white_space);
            quad.set_texture_adjust(0., 0., 0., 0.);
            quad.set_hsv(None);
            quad.set_underline(white_space);
            quad.set_has_color(false);
            quad.set_cursor(white_space);
            quad.set_cursor_color(rgbcolor_to_window_color(background_color));
        }

        {
            let mut quad = quads.background_image();
            let white_space = gl_state.util_sprites.white_space.texture_coords();
            quad.set_underline(white_space);
            quad.set_cursor(white_space);

            let background_image_alpha = (config.window_background_opacity * 255.0) as u8;
            let color = rgbcolor_alpha_to_window_color(palette.background, background_image_alpha);

            if let Some(im) = self.window_background.as_ref() {
                let (sprite, next_due) =
                    gl_state.glyph_cache.borrow_mut().cached_image(im, None)?;
                self.update_next_frame_time(next_due);
                quad.set_texture(sprite.texture_coords());
                quad.set_is_background_image();
            } else {
                quad.set_texture(white_space);
                quad.set_is_background();
            }
            quad.set_texture_adjust(0., 0., 0., 0.);
            quad.set_hsv(config.window_background_image_hsb);
            quad.set_cursor_color(color);
            quad.set_fg_color(color);
            quad.set_underline_color(color);
            quad.set_bg_color(color);
        }

        let selrange = self.selection(pos.pane.pane_id()).range.clone();

        let start = Instant::now();
        let selection_fg = rgbcolor_to_window_color(palette.selection_fg);
        let selection_bg = rgbcolor_to_window_color(palette.selection_bg);
        let cursor_fg = rgbcolor_to_window_color(if self.config.force_reverse_video_cursor {
            palette.background
        } else {
            palette.cursor_fg
        });
        let cursor_bg = rgbcolor_to_window_color(if self.config.force_reverse_video_cursor {
            palette.foreground
        } else {
            palette.cursor_bg
        });
        for (line_idx, line) in lines.iter().enumerate() {
            let stable_row = stable_top + line_idx as StableRowIndex;

            let selrange = selrange.map_or(0..0, |sel| sel.cols_for_row(stable_row));

            self.render_screen_line_opengl(
                RenderScreenLineOpenGLParams {
                    line_idx: line_idx + first_line_offset,
                    stable_line_idx: Some(stable_row),
                    line: &line,
                    selection: selrange,
                    cursor: &cursor,
                    palette: &palette,
                    dims: &dims,
                    config: &config,
                    cursor_border_color,
                    foreground,
                    pos,
                    is_active: pos.is_active,
                    selection_fg,
                    selection_bg,
                    cursor_fg,
                    cursor_bg,
                },
                &mut quads,
            )?;
        }
        log::trace!("lines elapsed {:?}", start.elapsed());

        let start = Instant::now();
        drop(quads);
        log::trace!("quad drop elapsed {:?}", start.elapsed());

        Ok(())
    }

    pub fn call_draw(&mut self, frame: &mut glium::Frame) -> anyhow::Result<()> {
        let gl_state = self.render_state.as_ref().unwrap();
        let mut vb = gl_state.glyph_vertex_buffer.borrow_mut();

        let tex = gl_state.glyph_cache.borrow().atlas.texture();
        let projection = euclid::Transform3D::<f32, f32, f32>::ortho(
            -(self.dimensions.pixel_width as f32) / 2.0,
            self.dimensions.pixel_width as f32 / 2.0,
            self.dimensions.pixel_height as f32 / 2.0,
            -(self.dimensions.pixel_height as f32) / 2.0,
            -1.0,
            1.0,
        )
        .to_arrays_transposed();

        let alpha_blending = glium::DrawParameters {
            blend: glium::Blend {
                color: BlendingFunction::Addition {
                    source: LinearBlendingFactor::SourceAlpha,
                    destination: LinearBlendingFactor::OneMinusSourceAlpha,
                },
                alpha: BlendingFunction::Addition {
                    source: LinearBlendingFactor::One,
                    destination: LinearBlendingFactor::OneMinusSourceAlpha,
                },
                constant_value: (0.0, 0.0, 0.0, 0.0),
            },

            ..Default::default()
        };

        // Clamp and use the nearest texel rather than interpolate.
        // This prevents things like the box cursor outlines from
        // being randomly doubled in width or height
        let atlas_nearest_sampler = Sampler::new(&*tex)
            .wrap_function(SamplerWrapFunction::Clamp)
            .magnify_filter(MagnifySamplerFilter::Nearest)
            .minify_filter(MinifySamplerFilter::Nearest);

        let atlas_linear_sampler = Sampler::new(&*tex)
            .wrap_function(SamplerWrapFunction::Clamp)
            .magnify_filter(MagnifySamplerFilter::Linear)
            .minify_filter(MinifySamplerFilter::Linear);

        let foreground_text_hsb = self.config.foreground_text_hsb;
        let foreground_text_hsb = (
            foreground_text_hsb.hue,
            foreground_text_hsb.saturation,
            foreground_text_hsb.brightness,
        );

        // Pass 1: Draw backgrounds
        frame.draw(
            &vb.bufs[vb.index],
            &gl_state.glyph_index_buffer,
            &gl_state.background_prog,
            &uniform! {
                projection: projection,
                atlas_linear_sampler:  atlas_linear_sampler,
                foreground_text_hsb: foreground_text_hsb,
            },
            &alpha_blending,
        )?;

        // Pass 2: strikethrough and underline
        frame.draw(
            &vb.bufs[vb.index],
            &gl_state.glyph_index_buffer,
            &gl_state.line_prog,
            &uniform! {
                projection: projection,
                atlas_nearest_sampler:  atlas_nearest_sampler,
                atlas_linear_sampler:  atlas_linear_sampler,
                foreground_text_hsb: foreground_text_hsb,
            },
            &alpha_blending,
        )?;

        // Use regular alpha blending when we draw the glyphs!
        // This is trying to avoid an issue that is most prevalent
        // on Wayland and X11.  If our glyph pixels end up with alpha
        // that isn't 1.0 then the window behind can cause the resultant
        // text to appear brighter or more bold, especially with a
        // 100% white window behind.
        //
        // To avoid this, for the computed alpha channel, instruct
        // the render phase to pick a larger alpha value.
        // There doesn't appear to be a way to tell it to set the
        // result to constant=1.0, only to influence the factors
        // in the equation:
        //
        // alpha = src_alpha * src_factor + dest_alpha * dest_factor.
        //
        // src_alpha comes from the glyph we are rendering.
        // dest_alpha comes from the background it is rendering over.
        // src_factor from alpha.source below
        // dest_factor from alpha.destination below
        //
        // If you're here troubleshooting this, please see:
        // <https://github.com/wez/wezterm/issues/413>
        // <https://github.com/wez/wezterm/issues/470>
        let blend_but_set_alpha_to_one = glium::DrawParameters {
            blend: glium::Blend {
                // Standard alpha-blending color
                color: BlendingFunction::Addition {
                    source: LinearBlendingFactor::SourceAlpha,
                    destination: LinearBlendingFactor::OneMinusSourceAlpha,
                },
                // Try to result in an alpha closer to 1.0
                alpha: BlendingFunction::Addition {
                    source: LinearBlendingFactor::One,
                    destination: LinearBlendingFactor::One,
                },
                constant_value: (0.0, 0.0, 0.0, 0.0),
            },
            ..Default::default()
        };

        // Pass 3: Draw glyphs
        frame.draw(
            &vb.bufs[vb.index],
            &gl_state.glyph_index_buffer,
            &gl_state.glyph_prog,
            &uniform! {
                projection: projection,
                atlas_nearest_sampler:  atlas_nearest_sampler,
                atlas_linear_sampler:  atlas_linear_sampler,
                foreground_text_hsb: foreground_text_hsb,
            },
            &blend_but_set_alpha_to_one,
        )?;

        vb.index += 1;
        if vb.index >= 3 {
            vb.index = 0;
        }

        Ok(())
    }

    pub fn paint_split_opengl(
        &mut self,
        split: &PositionedSplit,
        pane: &Rc<dyn Pane>,
    ) -> anyhow::Result<()> {
        let gl_state = self.render_state.as_ref().unwrap();
        let mut vb = gl_state.glyph_vertex_buffer.borrow_mut();
        let mut quads = gl_state.quads.map(&mut vb);
        let config = &self.config;
        let text = if split.direction == SplitDirection::Horizontal {
            "│"
        } else {
            "─"
        };
        let palette = pane.palette();
        let foreground = rgbcolor_to_window_color(palette.split);
        let background = rgbcolor_alpha_to_window_color(
            palette.background,
            if self.window_background.is_some() || config.window_background_opacity != 1.0 {
                0x00
            } else {
                (config.text_background_opacity * 255.0) as u8
            },
        );

        let style = self.fonts.match_style(&config, &CellAttributes::default());
        let glyph_info = {
            let key = BorrowedShapeCacheKey { style, text };
            match self.lookup_cached_shape(&key) {
                Some(Ok(info)) => info,
                Some(Err(err)) => return Err(err),
                None => {
                    let font = self.fonts.resolve_font(style)?;
                    let window = self.window.as_ref().unwrap().clone();
                    match font.shape(text, || Self::invalidate_post_font_resolve(window)) {
                        Ok(info) => {
                            let line = Line::from_text(&text, &CellAttributes::default());
                            let clusters = line.cluster();
                            let glyphs = self.glyph_infos_to_glyphs(
                                &clusters[0],
                                &line,
                                &style,
                                &mut gl_state.glyph_cache.borrow_mut(),
                                &info,
                            )?;
                            let shaped = ShapedInfo::process(
                                &self.render_metrics,
                                &clusters[0],
                                &info,
                                &glyphs,
                            );
                            self.shape_cache
                                .borrow_mut()
                                .put(key.to_owned(), Ok(Rc::new(shaped)));
                            self.lookup_cached_shape(&key).unwrap().unwrap()
                        }
                        Err(err) => {
                            if err.root_cause().downcast_ref::<ClearShapeCache>().is_some() {
                                return Err(err);
                            }

                            let res = anyhow!("shaper error: {}", err);
                            self.shape_cache.borrow_mut().put(key.to_owned(), Err(err));
                            return Err(res);
                        }
                    }
                }
            }
        };
        let first_row_offset = if self.show_tab_bar { 1 } else { 0 };

        for info in glyph_info.iter() {
            let glyph = &info.glyph;
            let left = info.pos.x_offset.get() as f32 + info.pos.bearing_x;
            let top = ((PixelLength::new(self.render_metrics.cell_size.height as f64)
                + self.render_metrics.descender)
                - (glyph.y_offset + glyph.bearing_y))
                .get() as f32;

            let texture = glyph
                .texture
                .as_ref()
                .unwrap_or(&gl_state.util_sprites.white_space);
            let underline_tex_rect = gl_state.util_sprites.white_space.texture_coords();

            let x_y_iter: Box<dyn Iterator<Item = (usize, usize)>> = if split.direction
                == SplitDirection::Horizontal
            {
                Box::new(std::iter::repeat(split.left).zip(split.top..split.top + split.size))
            } else {
                Box::new((split.left..split.left + split.size).zip(std::iter::repeat(split.top)))
            };
            for (x, y) in x_y_iter {
                let slice = SpriteSlice {
                    cell_idx: 0,
                    num_cells: info.pos.num_cells as usize,
                    cell_width: self.render_metrics.cell_size.width as usize,
                    scale: glyph.scale as f32,
                    left_offset: left,
                };

                let pixel_rect = slice.pixel_rect(texture);
                let texture_rect = texture.texture.to_texture_coords(pixel_rect);

                let bottom = (pixel_rect.size.height as f32 * glyph.scale as f32) + top
                    - self.render_metrics.cell_size.height as f32;
                let right = pixel_rect.size.width as f32 + left
                    - self.render_metrics.cell_size.width as f32;

                let mut quad = match quads.cell(x, y + first_row_offset) {
                    Ok(quad) => quad,
                    Err(_) => break,
                };

                quad.set_fg_color(foreground);
                quad.set_underline_color(foreground);
                quad.set_bg_color(background);
                quad.set_hsv(None);
                quad.set_texture(texture_rect);
                quad.set_texture_adjust(left, top, right, bottom);
                quad.set_underline(underline_tex_rect);
                quad.set_has_color(glyph.has_color);
                quad.set_cursor(underline_tex_rect);
                quad.set_cursor_color(background);
            }
        }
        Ok(())
    }

    pub fn paint_opengl_pass(&mut self) -> anyhow::Result<()> {
        let panes = self.get_panes_to_render();

        if let Some(pane) = self.get_active_pane_or_overlay() {
            let splits = self.get_splits();
            for split in &splits {
                self.paint_split_opengl(split, &pane)?;
            }
        }

        for pos in panes {
            if pos.is_active {
                self.update_text_cursor(&pos.pane);
            }
            self.paint_pane_opengl(&pos)?;
        }

        Ok(())
    }

    fn invalidate_post_font_resolve(window: ::window::Window) {
        promise::spawn::spawn_into_main_thread(async move {
            window
                .apply(move |tw, _| {
                    if let Some(tw) = tw.downcast_mut::<Self>() {
                        tw.shape_cache.borrow_mut().clear();
                        tw.window.as_ref().unwrap().invalidate();
                    }
                    Ok(())
                })
                .await
        })
        .detach();
    }

    /// "Render" a line of the terminal screen into the vertex buffer.
    /// This is nominally a matter of setting the fg/bg color and the
    /// texture coordinates for a given glyph.  There's a little bit
    /// of extra complexity to deal with multi-cell glyphs.
    pub fn render_screen_line_opengl(
        &self,
        params: RenderScreenLineOpenGLParams,
        quads: &mut MappedQuads,
    ) -> anyhow::Result<()> {
        let gl_state = self.render_state.as_ref().unwrap();

        let num_cols = params.dims.cols;

        let hsv = if params.is_active {
            None
        } else {
            Some(params.config.inactive_pane_hsb)
        };

        let window_is_transparent =
            self.window_background.is_some() || params.config.window_background_opacity != 1.0;

        let white_space = gl_state.util_sprites.white_space.texture_coords();

        // Pre-set the row with the whitespace glyph.
        // This is here primarily because clustering/shaping can cause the line updates
        // to skip setting a quad that is logically obscured by a double-wide glyph.
        // If eg: scrolling the viewport causes the pair of quads to change from two
        // individual cells to a single double-wide cell then we might leave the second
        // one of the pair with the glyph from the prior viewport position.
        for cell_idx in 0..num_cols {
            let mut quad =
                match quads.cell(cell_idx + params.pos.left, params.line_idx + params.pos.top) {
                    Ok(quad) => quad,
                    Err(_) => break,
                };

            quad.set_texture(white_space);
            quad.set_texture_adjust(0., 0., 0., 0.);
            quad.set_underline(white_space);
            quad.set_cursor(white_space);
        }

        // Break the line into clusters of cells with the same attributes
        let cell_clusters = params.line.cluster();

        let mut last_cell_idx = 0;

        for cluster in &cell_clusters {
            let attrs = &cluster.attrs;

            let is_highlited_hyperlink = match (attrs.hyperlink(), &self.current_highlight) {
                (Some(ref this), &Some(ref highlight)) => **this == *highlight,
                _ => false,
            };
            let style = self.fonts.match_style(params.config, attrs);
            // underline and strikethrough
            let underline_tex_rect = gl_state
                .glyph_cache
                .borrow_mut()
                .cached_line_sprite(
                    is_highlited_hyperlink,
                    attrs.strikethrough(),
                    attrs.underline(),
                    attrs.overline(),
                )?
                .texture_coords();

            let bg_is_default = attrs.background == ColorAttribute::Default;
            let bg_color = params.palette.resolve_bg(attrs.background);

            fn resolve_fg_color_attr(
                attrs: &CellAttributes,
                fg: &ColorAttribute,
                params: &RenderScreenLineOpenGLParams,
                style: &config::TextStyle,
            ) -> RgbColor {
                match fg {
                    wezterm_term::color::ColorAttribute::Default => {
                        if let Some(fg) = style.foreground {
                            fg
                        } else {
                            params.palette.resolve_fg(attrs.foreground)
                        }
                    }
                    wezterm_term::color::ColorAttribute::PaletteIndex(idx)
                        if *idx < 8 && params.config.bold_brightens_ansi_colors =>
                    {
                        // For compatibility purposes, switch to a brighter version
                        // of one of the standard ANSI colors when Bold is enabled.
                        // This lifts black to dark grey.
                        let idx = if attrs.intensity() == wezterm_term::Intensity::Bold {
                            *idx + 8
                        } else {
                            *idx
                        };
                        params
                            .palette
                            .resolve_fg(wezterm_term::color::ColorAttribute::PaletteIndex(idx))
                    }
                    _ => params.palette.resolve_fg(*fg),
                }
            }
            let fg_color = resolve_fg_color_attr(&attrs, &attrs.foreground, &params, &style);

            let (fg_color, bg_color, bg_is_default) = {
                let mut fg = fg_color;
                let mut bg = bg_color;
                let mut bg_default = bg_is_default;

                if attrs.reverse() {
                    std::mem::swap(&mut fg, &mut bg);
                    bg_default = false;
                }

                (fg, bg, bg_default)
            };

            let glyph_color = rgbcolor_to_window_color(fg_color);
            let underline_color = match attrs.underline_color() {
                ColorAttribute::Default => fg_color,
                c => resolve_fg_color_attr(&attrs, &c, &params, &style),
            };
            let underline_color = rgbcolor_to_window_color(underline_color);

            let bg_color = rgbcolor_alpha_to_window_color(
                bg_color,
                if window_is_transparent && bg_is_default {
                    0x00
                } else {
                    (params.config.text_background_opacity * 255.0) as u8
                },
            );

            // Shape the printable text from this cluster
            let glyph_info = {
                let key = BorrowedShapeCacheKey {
                    style,
                    text: &cluster.text,
                };
                match self.lookup_cached_shape(&key) {
                    Some(Ok(info)) => info,
                    Some(Err(err)) => return Err(err),
                    None => {
                        let font = self.fonts.resolve_font(style)?;
                        let window = self.window.as_ref().unwrap().clone();
                        match font
                            .shape(&cluster.text, || Self::invalidate_post_font_resolve(window))
                        {
                            Ok(info) => {
                                let glyphs = self.glyph_infos_to_glyphs(
                                    cluster,
                                    &params.line,
                                    &style,
                                    &mut gl_state.glyph_cache.borrow_mut(),
                                    &info,
                                )?;
                                let shaped = ShapedInfo::process(
                                    &self.render_metrics,
                                    cluster,
                                    &info,
                                    &glyphs,
                                );

                                self.shape_cache
                                    .borrow_mut()
                                    .put(key.to_owned(), Ok(Rc::new(shaped)));
                                self.lookup_cached_shape(&key).unwrap().unwrap()
                            }
                            Err(err) => {
                                if err.root_cause().downcast_ref::<ClearShapeCache>().is_some() {
                                    return Err(err);
                                }

                                let res = anyhow!("shaper error: {}", err);
                                self.shape_cache.borrow_mut().put(key.to_owned(), Err(err));
                                return Err(res);
                            }
                        }
                    }
                }
            };

            for info in glyph_info.iter() {
                let cell_idx = cluster.byte_to_cell_idx[info.pos.cluster as usize];
                let glyph = &info.glyph;

                let top = ((PixelLength::new(self.render_metrics.cell_size.height as f64)
                    + self.render_metrics.descender)
                    - (glyph.y_offset + glyph.bearing_y))
                    .get() as f32;

                // We use this to remember the `left` offset value to use for glyph_idx > 0
                let mut slice_left = 0.;

                // Iterate each cell that comprises this glyph.  There is usually
                // a single cell per glyph but combining characters, ligatures
                // and emoji can be 2 or more cells wide.
                for glyph_idx in 0..info.pos.num_cells as usize {
                    let cell_idx = cell_idx + glyph_idx;

                    if cell_idx >= num_cols {
                        // terminal line data is wider than the window.
                        // This happens for example while live resizing the window
                        // smaller than the terminal.
                        break;
                    }

                    last_cell_idx = cell_idx;

                    let ComputeCellFgBgResult {
                        fg_color: glyph_color,
                        bg_color,
                        cursor_shape,
                    } = self.compute_cell_fg_bg(ComputeCellFgBgParams {
                        stable_line_idx: params.stable_line_idx,
                        cell_idx,
                        cursor: params.cursor,
                        selection: &params.selection,
                        fg_color: glyph_color,
                        bg_color,
                        palette: params.palette,
                        is_active_pane: params.pos.is_active,
                        config: params.config,
                        selection_fg: params.selection_fg,
                        selection_bg: params.selection_bg,
                        cursor_fg: params.cursor_fg,
                        cursor_bg: params.cursor_bg,
                    });

                    if let Some(image) = attrs.image() {
                        self.populate_image_quad(
                            image,
                            gl_state,
                            quads,
                            cell_idx,
                            &params,
                            hsv,
                            cursor_shape,
                            glyph_color,
                            underline_color,
                            bg_color,
                            white_space,
                        )?;
                        continue;
                    }

                    if self.config.custom_block_glyphs && glyph_idx == 0 {
                        if let Some(block) = BlockKey::from_cell(&params.line.cells()[cell_idx]) {
                            self.populate_block_quad(
                                block,
                                gl_state,
                                quads,
                                cell_idx,
                                &params,
                                hsv,
                                cursor_shape,
                                glyph_color,
                                underline_color,
                                bg_color,
                                white_space,
                            )?;
                            continue;
                        }
                    }

                    let texture = glyph
                        .texture
                        .as_ref()
                        .unwrap_or(&gl_state.util_sprites.white_space);

                    let left = info.pos.x_offset.get() as f32 + info.pos.bearing_x;
                    let slice = SpriteSlice {
                        cell_idx: glyph_idx,
                        num_cells: info.pos.num_cells as usize,
                        cell_width: self.render_metrics.cell_size.width as usize,
                        scale: glyph.scale as f32,
                        left_offset: left,
                    };

                    let pixel_rect = slice.pixel_rect(texture);
                    let texture_rect = texture.texture.to_texture_coords(pixel_rect);

                    let left = if glyph_idx == 0 { left } else { slice_left };
                    let bottom = (pixel_rect.size.height as f32 * glyph.scale as f32) + top
                        - self.render_metrics.cell_size.height as f32;
                    let right = pixel_rect.size.width as f32 + left
                        - self.render_metrics.cell_size.width as f32;

                    // Save the `right` position; we'll use it for the `left` adjust for
                    // the next slice that comprises this glyph.
                    // This is important because some glyphs (eg: 현재 브랜치) can have
                    // fractional advance/offset positions that leave one half slightly
                    // out of alignment with the other if we were to simply force the
                    // `left` value to be 0 when glyph_idx > 0.
                    slice_left = right;

                    let mut quad = match quads
                        .cell(cell_idx + params.pos.left, params.line_idx + params.pos.top)
                    {
                        Ok(quad) => quad,
                        Err(_) => break,
                    };

                    quad.set_fg_color(glyph_color);
                    quad.set_bg_color(bg_color);
                    quad.set_texture(texture_rect);
                    quad.set_texture_adjust(left, top, right, bottom);
                    quad.set_underline(underline_tex_rect);
                    quad.set_underline_color(underline_color);
                    quad.set_hsv(hsv);
                    quad.set_has_color(glyph.has_color);
                    quad.set_cursor(
                        gl_state
                            .util_sprites
                            .cursor_sprite(cursor_shape)
                            .texture_coords(),
                    );
                    quad.set_cursor_color(params.cursor_border_color);
                }
            }
        }

        // Clear any remaining cells to the right of the clusters we
        // found above, otherwise we leave artifacts behind.  The easiest
        // reproduction for the artifacts is to maximize the window and
        // open a vim split horizontally.  Backgrounding vim would leave
        // the right pane with its prior contents instead of showing the
        // cleared lines from the shell in the main screen.

        let bg_color = rgbcolor_alpha_to_window_color(
            params.palette.resolve_bg(ColorAttribute::Default),
            if window_is_transparent {
                0x00
            } else {
                (params.config.text_background_opacity * 255.0) as u8
            },
        );

        for cell_idx in last_cell_idx + 1..num_cols {
            // Even though we don't have a cell for these, they still
            // hold the cursor or the selection so we need to compute
            // the colors in the usual way.

            let ComputeCellFgBgResult {
                fg_color: glyph_color,
                bg_color,
                cursor_shape,
            } = self.compute_cell_fg_bg(ComputeCellFgBgParams {
                stable_line_idx: params.stable_line_idx,
                cell_idx,
                cursor: params.cursor,
                selection: &params.selection,
                fg_color: params.foreground,
                bg_color,
                palette: params.palette,
                is_active_pane: params.pos.is_active,
                config: params.config,
                selection_fg: params.selection_fg,
                selection_bg: params.selection_bg,
                cursor_fg: params.cursor_fg,
                cursor_bg: params.cursor_bg,
            });

            let mut quad =
                match quads.cell(cell_idx + params.pos.left, params.line_idx + params.pos.top) {
                    Ok(quad) => quad,
                    Err(_) => break,
                };

            quad.set_bg_color(bg_color);
            quad.set_fg_color(glyph_color);
            quad.set_underline_color(glyph_color);
            quad.set_texture(white_space);
            quad.set_texture_adjust(0., 0., 0., 0.);
            quad.set_underline(white_space);
            quad.set_has_color(false);
            quad.set_hsv(hsv);
            quad.set_cursor(
                gl_state
                    .util_sprites
                    .cursor_sprite(cursor_shape)
                    .texture_coords(),
            );
            quad.set_cursor_color(params.cursor_border_color);
        }

        Ok(())
    }

    pub fn populate_block_quad(
        &self,
        block: BlockKey,
        gl_state: &RenderState,
        quads: &mut MappedQuads,
        cell_idx: usize,
        params: &RenderScreenLineOpenGLParams,
        hsv: Option<config::HsbTransform>,
        cursor_shape: Option<CursorShape>,
        glyph_color: LinearRgba,
        underline_color: LinearRgba,
        bg_color: LinearRgba,
        white_space: TextureRect,
    ) -> anyhow::Result<()> {
        let sprite = gl_state
            .glyph_cache
            .borrow_mut()
            .cached_block(block)?
            .texture_coords();

        let mut quad =
            match quads.cell(cell_idx + params.pos.left, params.line_idx + params.pos.top) {
                Ok(quad) => quad,
                Err(_) => return Ok(()),
            };

        quad.set_hsv(hsv);
        quad.set_fg_color(glyph_color);
        quad.set_underline_color(underline_color);
        quad.set_bg_color(bg_color);
        quad.set_texture(sprite);
        quad.set_texture_adjust(0., 0., 0., 0.);
        quad.set_underline(white_space);
        quad.set_has_color(false);
        quad.set_cursor(
            gl_state
                .util_sprites
                .cursor_sprite(cursor_shape)
                .texture_coords(),
        );
        quad.set_cursor_color(params.cursor_border_color);

        Ok(())
    }

    /// Render iTerm2 style image attributes
    pub fn populate_image_quad(
        &self,
        image: &termwiz::image::ImageCell,
        gl_state: &RenderState,
        quads: &mut MappedQuads,
        cell_idx: usize,
        params: &RenderScreenLineOpenGLParams,
        hsv: Option<config::HsbTransform>,
        cursor_shape: Option<CursorShape>,
        glyph_color: LinearRgba,
        underline_color: LinearRgba,
        bg_color: LinearRgba,
        white_space: TextureRect,
    ) -> anyhow::Result<()> {
        let padding = self
            .render_metrics
            .cell_size
            .height
            .max(self.render_metrics.cell_size.width) as usize;
        let padding = if padding.is_power_of_two() {
            padding
        } else {
            padding.next_power_of_two()
        };

        let (sprite, next_due) = gl_state
            .glyph_cache
            .borrow_mut()
            .cached_image(image.image_data(), Some(padding))?;
        self.update_next_frame_time(next_due);
        let width = sprite.coords.size.width;
        let height = sprite.coords.size.height;

        let top_left = image.top_left();
        let bottom_right = image.bottom_right();

        // We *could* call sprite.texture.to_texture_coords() here,
        // but since that takes integer pixel coordinates, we'd
        // lose precision and end up with visual artifacts.
        // Instead, we compute the texture coords here in floating point.

        let texture_width = sprite.texture.width() as f32;
        let texture_height = sprite.texture.height() as f32;
        let origin = TextureCoord::new(
            (sprite.coords.origin.x as f32 + (*top_left.x * width as f32)) / texture_width,
            (sprite.coords.origin.y as f32 + (*top_left.y * height as f32)) / texture_height,
        );

        let size = TextureSize::new(
            (*bottom_right.x - *top_left.x) * width as f32 / texture_width,
            (*bottom_right.y - *top_left.y) * height as f32 / texture_height,
        );

        let texture_rect = TextureRect::new(origin, size);

        let mut quad =
            match quads.cell(cell_idx + params.pos.left, params.line_idx + params.pos.top) {
                Ok(quad) => quad,
                Err(_) => return Ok(()),
            };

        quad.set_hsv(hsv);
        quad.set_fg_color(glyph_color);
        quad.set_underline_color(underline_color);
        quad.set_bg_color(bg_color);
        quad.set_texture(texture_rect);
        quad.set_texture_adjust(0., 0., 0., 0.);
        quad.set_underline(white_space);
        quad.set_has_color(true);
        quad.set_cursor(
            gl_state
                .util_sprites
                .cursor_sprite(cursor_shape)
                .texture_coords(),
        );
        quad.set_cursor_color(params.cursor_border_color);

        Ok(())
    }

    pub fn compute_cell_fg_bg(&self, params: ComputeCellFgBgParams) -> ComputeCellFgBgResult {
        let selected = params.selection.contains(&params.cell_idx);

        let is_cursor =
            params.stable_line_idx == Some(params.cursor.y) && params.cursor.x == params.cell_idx;

        let (cursor_shape, visibility) =
            if is_cursor && params.cursor.visibility == CursorVisibility::Visible {
                // This logic figures out whether the cursor is visible or not.
                // If the cursor is explicitly hidden then it is obviously not
                // visible.
                // If the cursor is set to a blinking mode then we are visible
                // depending on the current time.
                let shape = params
                    .config
                    .default_cursor_style
                    .effective_shape(params.cursor.shape);
                // Work out the blinking shape if its a blinking cursor and it hasn't been disabled
                // and the window is focused.
                let blinking = params.is_active_pane
                    && shape.is_blinking()
                    && params.config.cursor_blink_rate != 0
                    && self.focused.is_some();
                if blinking {
                    // Divide the time since we last moved by the blink rate.
                    // If the result is even then the cursor is "on", else it
                    // is "off"
                    let now = std::time::Instant::now();
                    let milli_uptime = now
                        .duration_since(self.prev_cursor.last_cursor_movement())
                        .as_millis();
                    let ticks = milli_uptime / params.config.cursor_blink_rate as u128;
                    (
                        shape,
                        if (ticks & 1) == 0 {
                            CursorVisibility::Visible
                        } else {
                            CursorVisibility::Hidden
                        },
                    )
                } else {
                    (shape, CursorVisibility::Visible)
                }
            } else {
                (params.cursor.shape, CursorVisibility::Hidden)
            };

        let (fg_color, bg_color) = match (
            selected,
            self.focused.is_some() && params.is_active_pane,
            cursor_shape,
            visibility,
        ) {
            // Selected text overrides colors
            (true, _, _, CursorVisibility::Hidden) => (params.selection_fg, params.selection_bg),
            // Cursor cell overrides colors
            (_, true, CursorShape::BlinkingBlock, CursorVisibility::Visible)
            | (_, true, CursorShape::SteadyBlock, CursorVisibility::Visible) => {
                (params.cursor_fg, params.cursor_bg)
            }
            // Normally, render the cell as configured (or if the window is unfocused)
            _ => (params.fg_color, params.bg_color),
        };

        ComputeCellFgBgResult {
            fg_color,
            bg_color,
            cursor_shape: if visibility == CursorVisibility::Visible {
                Some(cursor_shape)
            } else {
                None
            },
        }
    }

    fn glyph_infos_to_glyphs(
        &self,
        cluster: &CellCluster,
        line: &Line,
        style: &TextStyle,
        glyph_cache: &mut GlyphCache<SrgbTexture2d>,
        infos: &[GlyphInfo],
    ) -> anyhow::Result<Vec<Rc<CachedGlyph<SrgbTexture2d>>>> {
        let mut glyphs = vec![];
        for info in infos {
            let cell_idx = cluster.byte_to_cell_idx[info.cluster as usize];
            let followed_by_space = match line.cells().get(cell_idx + 1) {
                Some(cell) => cell.str() == " ",
                None => false,
            };

            glyphs.push(glyph_cache.cached_glyph(info, &style, followed_by_space)?);
        }
        Ok(glyphs)
    }

    fn lookup_cached_shape(
        &self,
        key: &dyn ShapeCacheKeyTrait,
    ) -> Option<anyhow::Result<Rc<Vec<ShapedInfo<SrgbTexture2d>>>>> {
        match self.shape_cache.borrow_mut().get(key) {
            Some(Ok(info)) => Some(Ok(Rc::clone(info))),
            Some(Err(err)) => Some(Err(anyhow!("cached shaper error: {}", err))),
            None => None,
        }
    }

    pub fn clear_texture_atlas(&mut self) -> anyhow::Result<()> {
        log::trace!("clear_texture_atlas");
        self.shape_cache.borrow_mut().clear();
        if let Some(render_state) = self.render_state.as_mut() {
            render_state.clear_texture_atlas(&self.render_metrics)?;
        }
        Ok(())
    }

    pub fn recreate_texture_atlas(&mut self, size: Option<usize>) -> anyhow::Result<()> {
        self.shape_cache.borrow_mut().clear();
        if let Some(render_state) = self.render_state.as_mut() {
            render_state.recreate_texture_atlas(&self.fonts, &self.render_metrics, size)?;
        }
        Ok(())
    }
}

fn rgbcolor_to_window_color(color: RgbColor) -> LinearRgba {
    rgbcolor_alpha_to_window_color(color, 0xff)
}

fn rgbcolor_alpha_to_window_color(color: RgbColor, alpha: u8) -> LinearRgba {
    // Note `RgbColor` is intended to be SRGB, but in practice it appears
    // as though it is linear RGB, hence this is using with_rgba rather than
    // with_srgba.
    LinearRgba::with_rgba(color.red, color.green, color.blue, alpha)
}
