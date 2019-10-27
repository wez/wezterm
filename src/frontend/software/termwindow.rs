use super::quad::*;
use super::renderstate::*;
use super::utilsprites::RenderMetrics;
use crate::config::Config;
use crate::font::{FontConfiguration, FontSystemSelection};
use crate::frontend::guicommon::clipboard::SystemClipboard;
use crate::frontend::{front_end, gui_executor};
use crate::keyassignment::{KeyAssignment, KeyMap, SpawnTabDomain};
use crate::mux::renderable::Renderable;
use crate::mux::tab::{Tab, TabId};
use crate::mux::window::WindowId as MuxWindowId;
use crate::mux::Mux;
use ::window::bitmaps::atlas::{OutOfTextureSpace, SpriteSlice};
use ::window::bitmaps::Texture2d;
use ::window::glium::{uniform, Surface};
use ::window::*;
use failure::Fallible;
use std::any::Any;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;
use term::color::ColorPalette;
use term::{CursorPosition, Line, Underline};
use termwiz::color::RgbColor;

pub struct TermWindow {
    window: Option<Window>,
    fonts: Rc<FontConfiguration>,
    _config: Arc<Config>,
    dimensions: Dimensions,
    mux_window_id: MuxWindowId,
    render_metrics: RenderMetrics,
    render_state: RenderState,
    clipboard: Arc<dyn term::Clipboard>,
    keys: KeyMap,
}

struct Host<'a> {
    writer: &'a mut dyn std::io::Write,
    context: &'a dyn WindowOps,
    clipboard: &'a Arc<dyn term::Clipboard>,
}

impl<'a> term::TerminalHost for Host<'a> {
    fn writer(&mut self) -> &mut dyn std::io::Write {
        self.writer
    }

    fn get_clipboard(&mut self) -> Fallible<Arc<dyn term::Clipboard>> {
        Ok(Arc::clone(self.clipboard))
    }

    fn set_title(&mut self, title: &str) {
        self.context.set_title(title);
    }

    fn click_link(&mut self, link: &Arc<term::cell::Hyperlink>) {
        log::error!("clicking {}", link.uri());
        if let Err(err) = open::that(link.uri()) {
            log::error!("failed to open {}: {:?}", link.uri(), err);
        }
    }
}

impl WindowCallbacks for TermWindow {
    fn created(&mut self, window: &Window) {
        self.window.replace(window.clone());
    }

    fn can_close(&mut self) -> bool {
        // can_close triggers the current tab to be closed.
        // If we have no tabs left then we can close the whole window.
        // If we're in a weird state, then we allow the window to close too.
        let mux = Mux::get().unwrap();
        let tab = match mux.get_active_tab_for_window(self.mux_window_id) {
            Some(tab) => tab,
            None => return true,
        };
        mux.remove_tab(tab.tab_id());
        if let Some(mut win) = mux.get_window_mut(self.mux_window_id) {
            win.remove_by_id(tab.tab_id());
            return win.is_empty();
        };
        true
    }

    fn as_any(&mut self) -> &mut dyn Any {
        self
    }

    fn mouse_event(&mut self, event: &MouseEvent, context: &dyn WindowOps) {
        let mux = Mux::get().unwrap();
        let tab = match mux.get_active_tab_for_window(self.mux_window_id) {
            Some(tab) => tab,
            None => return,
        };

        use ::term::input::MouseButton as TMB;
        use ::term::input::MouseEventKind as TMEK;
        use ::window::MouseButtons as WMB;
        use ::window::MouseEventKind as WMEK;
        tab.mouse_event(
            term::MouseEvent {
                kind: match event.kind {
                    WMEK::Move => TMEK::Move,
                    WMEK::VertWheel(_)
                    | WMEK::HorzWheel(_)
                    | WMEK::DoubleClick(_)
                    | WMEK::Press(_) => TMEK::Press,
                    WMEK::Release(_) => TMEK::Release,
                },
                button: match event.kind {
                    WMEK::Release(ref press)
                    | WMEK::Press(ref press)
                    | WMEK::DoubleClick(ref press) => match press {
                        MousePress::Left => TMB::Left,
                        MousePress::Middle => TMB::Middle,
                        MousePress::Right => TMB::Right,
                    },
                    WMEK::Move => {
                        if event.mouse_buttons == WMB::LEFT {
                            TMB::Left
                        } else if event.mouse_buttons == WMB::RIGHT {
                            TMB::Right
                        } else if event.mouse_buttons == WMB::MIDDLE {
                            TMB::Middle
                        } else {
                            TMB::None
                        }
                    }
                    WMEK::VertWheel(amount) => {
                        if amount > 0 {
                            TMB::WheelUp(amount as usize)
                        } else {
                            TMB::WheelDown((-amount) as usize)
                        }
                    }
                    WMEK::HorzWheel(_) => TMB::None,
                },
                x: (event.x as isize / self.render_metrics.cell_size.width) as usize,
                y: (event.y as isize / self.render_metrics.cell_size.height) as i64,
                modifiers: window_mods_to_termwiz_mods(event.modifiers),
            },
            &mut Host {
                writer: &mut *tab.writer(),
                context,
                clipboard: &self.clipboard,
            },
        )
        .ok();

        match event.kind {
            WMEK::Move => {}
            _ => context.invalidate(),
        }

        // When hovering over a hyperlink, show an appropriate
        // mouse cursor to give the cue that it is clickable
        context.set_cursor(Some(if tab.renderer().current_highlight().is_some() {
            MouseCursor::Hand
        } else {
            MouseCursor::Text
        }));
    }

    fn resize(&mut self, dimensions: Dimensions) {
        if dimensions.pixel_width == 0 || dimensions.pixel_height == 0 {
            // on windows, this can happen when minimizing the window.
            // NOP!
            return;
        }
        self.scaling_changed(dimensions, self.fonts.get_font_scale());
    }

    fn key_event(&mut self, key: &KeyEvent, _context: &dyn WindowOps) -> bool {
        if !key.key_is_down {
            return false;
        }

        let mux = Mux::get().unwrap();
        if let Some(tab) = mux.get_active_tab_for_window(self.mux_window_id) {
            let modifiers = window_mods_to_termwiz_mods(key.modifiers);

            use ::termwiz::input::KeyCode as KC;
            use ::window::KeyCode as WK;

            let key_down = match key.key {
                WK::Char(c) => Some(KC::Char(c)),
                WK::Composed(ref s) => {
                    tab.writer().write_all(s.as_bytes()).ok();
                    return true;
                }
                WK::Function(f) => Some(KC::Function(f)),
                WK::LeftArrow => Some(KC::LeftArrow),
                WK::RightArrow => Some(KC::RightArrow),
                WK::UpArrow => Some(KC::UpArrow),
                WK::DownArrow => Some(KC::DownArrow),
                WK::Home => Some(KC::Home),
                WK::End => Some(KC::End),
                WK::PageUp => Some(KC::PageUp),
                WK::PageDown => Some(KC::PageDown),
                // TODO: more keys (eg: numpad!)
                _ => None,
            };

            if let Some(key) = key_down {
                if let Some(assignment) = self.keys.lookup(key, modifiers) {
                    self.perform_key_assignment(&tab, &assignment).ok();
                    return true;
                } else if tab.key_down(key, modifiers).is_ok() {
                    return true;
                }
            }
        }

        false
    }

    fn paint(&mut self, ctx: &mut dyn PaintContext) {
        let mux = Mux::get().unwrap();
        let tab = match mux.get_active_tab_for_window(self.mux_window_id) {
            Some(tab) => tab,
            None => {
                ctx.clear(Color::rgb(0, 0, 0));
                return;
            }
        };
        let start = std::time::Instant::now();
        if let Err(err) = self.paint_tab(&tab, ctx) {
            if let Some(&OutOfTextureSpace { size }) = err.downcast_ref::<OutOfTextureSpace>() {
                log::error!("out of texture space, allocating {}", size);
                if let Err(err) = self.recreate_texture_atlas(Some(size)) {
                    log::error!("failed recreate atlas with size {}: {}", size, err);
                    // Failed to increase the size.
                    // This might happen if a lot of images have been displayed in the
                    // terminal over time and we've hit a texture size limit.
                    // Let's just try recreating at the current size.
                    self.recreate_texture_atlas(None)
                        .expect("OutOfTextureSpace and failed to recreate atlas");
                }
                tab.renderer().make_all_lines_dirty();
                // Recursively initiate a new paint
                return self.paint(ctx);
            }
            log::error!("paint failed: {}", err);
        }
        log::debug!("paint_tab elapsed={:?}", start.elapsed());
        self.update_title();
    }

    fn paint_opengl(&mut self, frame: &mut glium::Frame) {
        let mux = Mux::get().unwrap();
        let tab = match mux.get_active_tab_for_window(self.mux_window_id) {
            Some(tab) => tab,
            None => {
                frame.clear_color(0., 0., 0., 1.);
                return;
            }
        };
        let start = std::time::Instant::now();
        if let Err(err) = self.paint_tab_opengl(&tab, frame) {
            if let Some(&OutOfTextureSpace { size }) = err.downcast_ref::<OutOfTextureSpace>() {
                log::error!("out of texture space, allocating {}", size);
                if let Err(err) = self.recreate_texture_atlas(Some(size)) {
                    log::error!("failed recreate atlas with size {}: {}", size, err);
                    // Failed to increase the size.
                    // This might happen if a lot of images have been displayed in the
                    // terminal over time and we've hit a texture size limit.
                    // Let's just try recreating at the current size.
                    self.recreate_texture_atlas(None)
                        .expect("OutOfTextureSpace and failed to recreate atlas");
                }
                tab.renderer().make_all_lines_dirty();
                // Recursively initiate a new paint
                return self.paint_opengl(frame);
            }
            log::error!("paint_tab_opengl failed: {}", err);
        }
        log::debug!("paint_tab_opengl elapsed={:?}", start.elapsed());
        self.update_title();
    }
}

impl TermWindow {
    pub fn new_window(
        config: &Arc<Config>,
        fontconfig: &Rc<FontConfiguration>,
        tab: &Rc<dyn Tab>,
        mux_window_id: MuxWindowId,
    ) -> Fallible<()> {
        log::error!(
            "TermWindow::new_window called with mux_window_id {}",
            mux_window_id
        );
        let (physical_rows, physical_cols) = tab.renderer().physical_dimensions();

        let render_metrics = RenderMetrics::new(fontconfig);

        let width = render_metrics.cell_size.width as usize * physical_cols;
        let height = render_metrics.cell_size.height as usize * physical_rows;

        const ATLAS_SIZE: usize = 4096;
        let render_state = RenderState::Software(SoftwareRenderState::new(
            fontconfig,
            &render_metrics,
            ATLAS_SIZE,
        )?);

        let window = Window::new_window(
            "wezterm",
            "wezterm",
            width,
            height,
            Box::new(Self {
                window: None,
                mux_window_id,
                _config: Arc::clone(config),
                fonts: Rc::clone(fontconfig),
                render_metrics,
                dimensions: Dimensions {
                    pixel_width: width,
                    pixel_height: height,
                    // This is the default dpi; we'll get a resize
                    // event to inform us of the true dpi if it is
                    // different from this value
                    dpi: 96,
                },
                render_state,
                clipboard: Arc::new(SystemClipboard::new()),
                keys: KeyMap::new(),
            }),
        )?;

        let cloned_window = window.clone();

        Connection::get().unwrap().schedule_timer(
            std::time::Duration::from_millis(35),
            move || {
                let mux = Mux::get().unwrap();
                if let Some(tab) = mux.get_active_tab_for_window(mux_window_id) {
                    if tab.renderer().has_dirty_lines() {
                        cloned_window.invalidate();
                    }
                } else {
                    cloned_window.close();
                }
            },
        );

        window.show();

        if super::is_opengl_enabled() {
            window.enable_opengl(|any, _window, maybe_ctx| {
                let mut termwindow = any.downcast_mut::<TermWindow>().expect("to be TermWindow");

                match maybe_ctx {
                    Ok(ctx) => {
                        match OpenGLRenderState::new(
                            ctx,
                            &termwindow.fonts,
                            &termwindow.render_metrics,
                            ATLAS_SIZE,
                            termwindow.dimensions.pixel_width,
                            termwindow.dimensions.pixel_height,
                        ) {
                            Ok(gl) => {
                                log::error!(
                                    "OpenGL initialized! {} {}",
                                    gl.context.get_opengl_renderer_string(),
                                    gl.context.get_opengl_version_string()
                                );
                                termwindow.render_state = RenderState::GL(gl);
                            }
                            Err(err) => {
                                log::error!("OpenGL init failed: {}", err);
                            }
                        }
                    }
                    Err(err) => log::error!("OpenGL init failed: {}", err),
                }
            });
        }

        Ok(())
    }

    fn recreate_texture_atlas(&mut self, size: Option<usize>) -> Fallible<()> {
        self.render_state
            .recreate_texture_atlas(&self.fonts, &self.render_metrics, size)
    }

    fn update_title(&mut self) {
        let mux = Mux::get().unwrap();
        let window = match mux.get_window(self.mux_window_id) {
            Some(window) => window,
            _ => return,
        };
        let num_tabs = window.len();

        if num_tabs == 0 {
            return;
        }
        let tab_no = window.get_active_idx();

        let title = match window.get_active() {
            Some(tab) => tab.get_title(),
            None => return,
        };

        drop(window);

        if let Some(window) = self.window.as_ref() {
            if num_tabs == 1 {
                window.set_title(&title);
            } else {
                window.set_title(&format!("[{}/{}] {}", tab_no + 1, num_tabs, title));
            }
        }
    }

    fn activate_tab(&mut self, tab_idx: usize) -> Fallible<()> {
        let mux = Mux::get().unwrap();
        let mut window = mux
            .get_window_mut(self.mux_window_id)
            .ok_or_else(|| failure::format_err!("no such window"))?;

        let max = window.len();
        if tab_idx < max {
            window.set_active(tab_idx);

            drop(window);
            self.update_title();
        }
        Ok(())
    }

    fn activate_tab_relative(&mut self, delta: isize) -> Fallible<()> {
        let mux = Mux::get().unwrap();
        let window = mux
            .get_window(self.mux_window_id)
            .ok_or_else(|| failure::format_err!("no such window"))?;

        let max = window.len();
        failure::ensure!(max > 0, "no more tabs");

        let active = window.get_active_idx() as isize;
        let tab = active + delta;
        let tab = if tab < 0 { max as isize + tab } else { tab };
        drop(window);
        self.activate_tab(tab as usize % max)
    }

    fn spawn_tab(&mut self, domain: &SpawnTabDomain) -> Fallible<TabId> {
        let rows = (self.dimensions.pixel_height as usize + 1)
            / self.render_metrics.cell_size.height as usize;
        let cols = (self.dimensions.pixel_width as usize + 1)
            / self.render_metrics.cell_size.width as usize;

        let size = portable_pty::PtySize {
            rows: rows as u16,
            cols: cols as u16,
            pixel_width: self.dimensions.pixel_width as u16,
            pixel_height: self.dimensions.pixel_height as u16,
        };

        let mux = Mux::get().unwrap();

        let domain = match domain {
            SpawnTabDomain::DefaultDomain => mux.default_domain().clone(),
            SpawnTabDomain::CurrentTabDomain => {
                let tab = match mux.get_active_tab_for_window(self.mux_window_id) {
                    Some(tab) => tab,
                    None => failure::bail!("window has no tabs?"),
                };
                mux.get_domain(tab.domain_id()).ok_or_else(|| {
                    failure::format_err!("current tab has unresolvable domain id!?")
                })?
            }
            SpawnTabDomain::Domain(id) => mux.get_domain(*id).ok_or_else(|| {
                failure::format_err!("spawn_tab called with unresolvable domain id!?")
            })?,
            SpawnTabDomain::DomainName(name) => mux.get_domain_by_name(&name).ok_or_else(|| {
                failure::format_err!("spawn_tab called with unresolvable domain name {}", name)
            })?,
        };
        let tab = domain.spawn(size, None, self.mux_window_id)?;
        let tab_id = tab.tab_id();

        let len = {
            let window = mux
                .get_window(self.mux_window_id)
                .ok_or_else(|| failure::format_err!("no such window!?"))?;
            window.len()
        };
        self.activate_tab(len - 1)?;
        Ok(tab_id)
    }

    fn perform_key_assignment(
        &mut self,
        tab: &Rc<dyn Tab>,
        assignment: &KeyAssignment,
    ) -> Fallible<()> {
        use KeyAssignment::*;
        match assignment {
            SpawnTab(spawn_where) => {
                self.spawn_tab(spawn_where)?;
            }
            SpawnWindow => {
                self.spawn_new_window();
            }
            ToggleFullScreen => {
                // self.toggle_full_screen(),
            }
            Copy => {
                // Nominally copy, but that is implicit, so NOP
            }
            Paste => {
                tab.trickle_paste(self.clipboard.get_contents()?)?;
            }
            ActivateTabRelative(n) => {
                self.activate_tab_relative(*n)?;
            }
            DecreaseFontSize => self.decrease_font_size(),
            IncreaseFontSize => self.increase_font_size(),
            ResetFontSize => self.reset_font_size(),
            ActivateTab(n) => {
                self.activate_tab(*n)?;
            }
            SendString(s) => tab.writer().write_all(s.as_bytes())?,
            Hide => {
                if let Some(w) = self.window.as_ref() {
                    w.hide();
                }
            }
            Show => {
                if let Some(w) = self.window.as_ref() {
                    w.show();
                }
            }
            CloseCurrentTab => self.close_current_tab(),
            Nop => {}
        };
        Ok(())
    }

    pub fn spawn_new_window(&mut self) {
        promise::Future::with_executor(gui_executor().unwrap(), move || {
            let mux = Mux::get().unwrap();
            let fonts = Rc::new(FontConfiguration::new(
                Arc::clone(mux.config()),
                FontSystemSelection::get_default(),
            ));
            let window_id = mux.new_empty_window();
            let tab =
                mux.default_domain()
                    .spawn(portable_pty::PtySize::default(), None, window_id)?;
            let front_end = front_end().expect("to be called on gui thread");
            front_end.spawn_new_window(mux.config(), &fonts, &tab, window_id)?;
            Ok(())
        });
    }

    #[allow(clippy::float_cmp)]
    fn scaling_changed(&mut self, dimensions: Dimensions, font_scale: f64) {
        let mux = Mux::get().unwrap();
        if let Some(window) = mux.get_window(self.mux_window_id) {
            if dimensions.dpi != self.dimensions.dpi || font_scale != self.fonts.get_font_scale() {
                self.fonts
                    .change_scaling(font_scale, dimensions.dpi as f64 / 96.);
                self.render_metrics = RenderMetrics::new(&self.fonts);

                self.recreate_texture_atlas(None)
                    .expect("failed to recreate atlas");
            }

            self.dimensions = dimensions;

            self.render_state
                .advise_of_window_size_change(
                    &self.render_metrics,
                    dimensions.pixel_width,
                    dimensions.pixel_height,
                )
                .expect("failed to advise of resize");

            let size = portable_pty::PtySize {
                rows: dimensions.pixel_height as u16 / self.render_metrics.cell_size.height as u16,
                cols: dimensions.pixel_width as u16 / self.render_metrics.cell_size.width as u16,
                pixel_height: dimensions.pixel_height as u16,
                pixel_width: dimensions.pixel_width as u16,
            };
            for tab in window.iter() {
                tab.resize(size).ok();
            }
        };
    }

    fn decrease_font_size(&mut self) {
        self.scaling_changed(self.dimensions, self.fonts.get_font_scale() * 0.9);
    }
    fn increase_font_size(&mut self) {
        self.scaling_changed(self.dimensions, self.fonts.get_font_scale() * 1.1);
    }
    fn reset_font_size(&mut self) {
        self.scaling_changed(self.dimensions, 1.);
    }

    fn close_current_tab(&mut self) {
        let mux = Mux::get().unwrap();
        let tab = match mux.get_active_tab_for_window(self.mux_window_id) {
            Some(tab) => tab,
            None => return,
        };
        mux.remove_tab(tab.tab_id());
        if let Some(mut win) = mux.get_window_mut(self.mux_window_id) {
            win.remove_by_id(tab.tab_id());
        }
        self.activate_tab_relative(0).ok();
    }

    fn paint_tab(&mut self, tab: &Rc<dyn Tab>, ctx: &mut dyn PaintContext) -> Fallible<()> {
        let palette = tab.palette();

        let mut term = tab.renderer();
        let cursor = term.get_cursor_position();

        {
            let dirty_lines = term.get_dirty_lines();

            for (line_idx, line, selrange) in dirty_lines {
                self.render_screen_line(ctx, line_idx, &line, selrange, &cursor, &*term, &palette)?;
            }
        }

        term.clean_dirty_lines();

        // Fill any marginal area below the last row
        let (num_rows, _num_cols) = term.physical_dimensions();
        let pixel_height_of_cells = num_rows * self.render_metrics.cell_size.height as usize;
        ctx.clear_rect(
            Rect::new(
                Point::new(0, pixel_height_of_cells as isize),
                Size::new(
                    self.dimensions.pixel_width as isize,
                    (self.dimensions.pixel_height - pixel_height_of_cells) as isize,
                ),
            ),
            rgbcolor_to_window_color(palette.background),
        );
        Ok(())
    }

    fn paint_tab_opengl(&mut self, tab: &Rc<dyn Tab>, frame: &mut glium::Frame) -> Fallible<()> {
        let palette = tab.palette();

        let background_color = palette.resolve_bg(term::color::ColorAttribute::Default);
        let (r, g, b, a) = background_color.to_tuple_rgba();
        frame.clear_color(r, g, b, a);

        let mut term = tab.renderer();
        let cursor = term.get_cursor_position();

        {
            let dirty_lines = term.get_dirty_lines();

            for (line_idx, line, selrange) in dirty_lines {
                self.render_screen_line_opengl(
                    line_idx, &line, selrange, &cursor, &*term, &palette,
                )?;
            }
        }

        let gl_state = self.render_state.opengl();
        let tex = gl_state.glyph_cache.borrow().atlas.texture();
        let projection = euclid::Transform3D::<f32, f32, f32>::ortho(
            -(self.dimensions.pixel_width as f32) / 2.0,
            self.dimensions.pixel_width as f32 / 2.0,
            self.dimensions.pixel_height as f32 / 2.0,
            -(self.dimensions.pixel_height as f32) / 2.0,
            -1.0,
            1.0,
        )
        .to_column_arrays();

        let draw_params = glium::DrawParameters {
            blend: glium::Blend::alpha_blending(),
            ..Default::default()
        };

        // Pass 1: Draw backgrounds, strikethrough and underline
        frame.draw(
            &*gl_state.glyph_vertex_buffer.borrow(),
            &gl_state.glyph_index_buffer,
            &gl_state.program,
            &uniform! {
                projection: projection,
                glyph_tex: &*tex,
                bg_and_line_layer: true,
            },
            &draw_params,
        )?;

        // Pass 2: Draw glyphs
        frame.draw(
            &*gl_state.glyph_vertex_buffer.borrow(),
            &gl_state.glyph_index_buffer,
            &gl_state.program,
            &uniform! {
                projection: projection,
                glyph_tex: &*tex,
                bg_and_line_layer: false,
            },
            &draw_params,
        )?;

        term.clean_dirty_lines();

        Ok(())
    }

    /// "Render" a line of the terminal screen into the vertex buffer.
    /// This is nominally a matter of setting the fg/bg color and the
    /// texture coordinates for a given glyph.  There's a little bit
    /// of extra complexity to deal with multi-cell glyphs.
    fn render_screen_line_opengl(
        &self,
        line_idx: usize,
        line: &Line,
        selection: Range<usize>,
        cursor: &CursorPosition,
        terminal: &dyn Renderable,
        palette: &ColorPalette,
    ) -> Fallible<()> {
        let gl_state = self.render_state.opengl();

        let (_num_rows, num_cols) = terminal.physical_dimensions();
        let mut vb = gl_state.glyph_vertex_buffer.borrow_mut();
        let mut vertices = {
            let per_line = num_cols * VERTICES_PER_CELL;
            let start_pos = line_idx * per_line;
            vb.slice_mut(start_pos..start_pos + per_line)
                .ok_or_else(|| failure::err_msg("we're confused about the screen size"))?
                .map()
        };

        let current_highlight = terminal.current_highlight();

        // Break the line into clusters of cells with the same attributes
        let cell_clusters = line.cluster();
        let mut last_cell_idx = 0;
        for cluster in cell_clusters {
            let attrs = &cluster.attrs;
            let is_highlited_hyperlink = match (&attrs.hyperlink, &current_highlight) {
                (&Some(ref this), &Some(ref highlight)) => this == highlight,
                _ => false,
            };
            let style = self.fonts.match_style(attrs);

            let bg_color = palette.resolve_bg(attrs.background);
            let fg_color = match attrs.foreground {
                term::color::ColorAttribute::Default => {
                    if let Some(fg) = style.foreground {
                        fg
                    } else {
                        palette.resolve_fg(attrs.foreground)
                    }
                }
                term::color::ColorAttribute::PaletteIndex(idx) if idx < 8 => {
                    // For compatibility purposes, switch to a brighter version
                    // of one of the standard ANSI colors when Bold is enabled.
                    // This lifts black to dark grey.
                    let idx = if attrs.intensity() == term::Intensity::Bold {
                        idx + 8
                    } else {
                        idx
                    };
                    palette.resolve_fg(term::color::ColorAttribute::PaletteIndex(idx))
                }
                _ => palette.resolve_fg(attrs.foreground),
            };

            let (fg_color, bg_color) = {
                let mut fg = fg_color;
                let mut bg = bg_color;

                if attrs.reverse() {
                    std::mem::swap(&mut fg, &mut bg);
                }

                (fg, bg)
            };

            let glyph_color = fg_color;
            let bg_color = bg_color;

            // Shape the printable text from this cluster
            let glyph_info = {
                let font = self.fonts.cached_font(style)?;
                let mut font = font.borrow_mut();
                font.shape(&cluster.text)?
            };

            for info in &glyph_info {
                let cell_idx = cluster.byte_to_cell_idx[info.cluster as usize];
                let glyph = gl_state
                    .glyph_cache
                    .borrow_mut()
                    .cached_glyph(info, style)?;

                let left = (glyph.x_offset + glyph.bearing_x) as f32;
                let top = ((self.render_metrics.cell_size.height as f64
                    + self.render_metrics.descender)
                    - (glyph.y_offset + glyph.bearing_y)) as f32;

                // underline and strikethrough
                let underline_tex_rect = gl_state
                    .util_sprites
                    .select_sprite(
                        is_highlited_hyperlink,
                        attrs.strikethrough(),
                        attrs.underline(),
                    )
                    .texture_coords();

                // Iterate each cell that comprises this glyph.  There is usually
                // a single cell per glyph but combining characters, ligatures
                // and emoji can be 2 or more cells wide.
                for glyph_idx in 0..info.num_cells as usize {
                    let cell_idx = cell_idx + glyph_idx;

                    if cell_idx >= num_cols {
                        // terminal line data is wider than the window.
                        // This happens for example while live resizing the window
                        // smaller than the terminal.
                        break;
                    }
                    last_cell_idx = cell_idx;

                    let (glyph_color, bg_color) = self.compute_cell_fg_bg(
                        line_idx,
                        cell_idx,
                        cursor,
                        &selection,
                        rgbcolor_to_window_color(glyph_color),
                        rgbcolor_to_window_color(bg_color),
                        palette,
                    );

                    if let Some(image) = attrs.image.as_ref() {
                        // Render iTerm2 style image attributes

                        if let Ok(sprite) = gl_state
                            .glyph_cache
                            .borrow_mut()
                            .cached_image(image.image_data())
                        {
                            let width = sprite.coords.size.width;
                            let height = sprite.coords.size.height;

                            let top_left = image.top_left();
                            let bottom_right = image.bottom_right();
                            let origin = Point::new(
                                sprite.coords.origin.x + (*top_left.x * width as f32) as isize,
                                sprite.coords.origin.y + (*top_left.y * height as f32) as isize,
                            );

                            let coords = Rect::new(
                                origin,
                                Size::new(
                                    ((*bottom_right.x - *top_left.x) * width as f32) as isize,
                                    ((*bottom_right.y - *top_left.y) * height as f32) as isize,
                                ),
                            );

                            let texture_rect = sprite.texture.to_texture_coords(coords);

                            let mut quad = Quad::for_cell(cell_idx, &mut vertices);

                            quad.set_fg_color(glyph_color);
                            quad.set_bg_color(bg_color);
                            quad.set_texture(texture_rect);
                            quad.set_texture_adjust(0., 0., 0., 0.);
                            quad.set_underline(gl_state.util_sprites.white_space.texture_coords());
                            quad.set_has_color(true);

                            continue;
                        }
                    }

                    let texture = glyph
                        .texture
                        .as_ref()
                        .unwrap_or(&gl_state.util_sprites.white_space);

                    let slice = SpriteSlice {
                        cell_idx: glyph_idx,
                        num_cells: info.num_cells as usize,
                        cell_width: self.render_metrics.cell_size.width as usize,
                        scale: glyph.scale as f32,
                        left_offset: left,
                    };

                    let pixel_rect = slice.pixel_rect(texture);
                    let texture_rect = texture.texture.to_texture_coords(pixel_rect);

                    let left = if glyph_idx == 0 { left } else { 0.0 };
                    let bottom = (pixel_rect.size.height as f32 * glyph.scale as f32) + top
                        - self.render_metrics.cell_size.height as f32;
                    let right = pixel_rect.size.width as f32 + left
                        - self.render_metrics.cell_size.width as f32;

                    let mut quad = Quad::for_cell(cell_idx, &mut vertices);

                    quad.set_fg_color(glyph_color);
                    quad.set_bg_color(bg_color);
                    quad.set_texture(texture_rect);
                    quad.set_texture_adjust(left, top, right, bottom);
                    quad.set_underline(underline_tex_rect);
                    quad.set_has_color(glyph.has_color);
                }
            }
        }

        // Clear any remaining cells to the right of the clusters we
        // found above, otherwise we leave artifacts behind.  The easiest
        // reproduction for the artifacts is to maximize the window and
        // open a vim split horizontally.  Backgrounding vim would leave
        // the right pane with its prior contents instead of showing the
        // cleared lines from the shell in the main screen.

        let white_space = gl_state.util_sprites.white_space.texture_coords();

        for cell_idx in last_cell_idx + 1..num_cols {
            // Even though we don't have a cell for these, they still
            // hold the cursor or the selection so we need to compute
            // the colors in the usual way.
            let (glyph_color, bg_color) = self.compute_cell_fg_bg(
                line_idx,
                cell_idx,
                cursor,
                &selection,
                rgbcolor_to_window_color(palette.foreground),
                rgbcolor_to_window_color(palette.background),
                palette,
            );

            let mut quad = Quad::for_cell(cell_idx, &mut vertices);

            quad.set_bg_color(bg_color);
            quad.set_fg_color(glyph_color);
            quad.set_texture(white_space);
            quad.set_texture_adjust(0., 0., 0., 0.);
            quad.set_underline(white_space);
            quad.set_has_color(false);
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn render_screen_line(
        &self,
        ctx: &mut dyn PaintContext,
        line_idx: usize,
        line: &Line,
        selection: Range<usize>,
        cursor: &CursorPosition,
        terminal: &dyn Renderable,
        palette: &ColorPalette,
    ) -> Fallible<()> {
        let (_num_rows, num_cols) = terminal.physical_dimensions();
        let current_highlight = terminal.current_highlight();

        // Break the line into clusters of cells with the same attributes
        let cell_clusters = line.cluster();
        let mut last_cell_idx = 0;
        for cluster in cell_clusters {
            let attrs = &cluster.attrs;
            let is_highlited_hyperlink = match (&attrs.hyperlink, &current_highlight) {
                (&Some(ref this), &Some(ref highlight)) => this == highlight,
                _ => false,
            };
            let style = self.fonts.match_style(attrs);

            let bg_color = palette.resolve_bg(attrs.background);
            let fg_color = match attrs.foreground {
                term::color::ColorAttribute::Default => {
                    if let Some(fg) = style.foreground {
                        fg
                    } else {
                        palette.resolve_fg(attrs.foreground)
                    }
                }
                term::color::ColorAttribute::PaletteIndex(idx) if idx < 8 => {
                    // For compatibility purposes, switch to a brighter version
                    // of one of the standard ANSI colors when Bold is enabled.
                    // This lifts black to dark grey.
                    let idx = if attrs.intensity() == term::Intensity::Bold {
                        idx + 8
                    } else {
                        idx
                    };
                    palette.resolve_fg(term::color::ColorAttribute::PaletteIndex(idx))
                }
                _ => palette.resolve_fg(attrs.foreground),
            };

            let (fg_color, bg_color) = {
                let mut fg = fg_color;
                let mut bg = bg_color;

                if attrs.reverse() {
                    std::mem::swap(&mut fg, &mut bg);
                }

                (fg, bg)
            };

            let glyph_color = rgbcolor_to_window_color(fg_color);
            let bg_color = rgbcolor_to_window_color(bg_color);

            // Shape the printable text from this cluster
            let glyph_info = {
                let font = self.fonts.cached_font(style)?;
                let mut font = font.borrow_mut();
                font.shape(&cluster.text)?
            };

            for info in &glyph_info {
                let cell_idx = cluster.byte_to_cell_idx[info.cluster as usize];
                let glyph = self.render_state.cached_software_glyph(info, style)?;

                let left = (glyph.x_offset + glyph.bearing_x) as f32;
                let top = ((self.render_metrics.cell_size.height as f64
                    + self.render_metrics.descender)
                    - (glyph.y_offset + glyph.bearing_y)) as f32;

                // underline and strikethrough
                // Figure out what we're going to draw for the underline.
                // If the current cell is part of the current URL highlight
                // then we want to show the underline.
                let underline = match (is_highlited_hyperlink, attrs.underline()) {
                    (true, Underline::None) => Underline::Single,
                    (_, underline) => underline,
                };

                // Iterate each cell that comprises this glyph.  There is usually
                // a single cell per glyph but combining characters, ligatures
                // and emoji can be 2 or more cells wide.
                for glyph_idx in 0..info.num_cells as usize {
                    let cell_idx = cell_idx + glyph_idx;

                    if cell_idx >= num_cols {
                        // terminal line data is wider than the window.
                        // This happens for example while live resizing the window
                        // smaller than the terminal.
                        break;
                    }
                    last_cell_idx = cell_idx;

                    let (glyph_color, bg_color) = self.compute_cell_fg_bg(
                        line_idx,
                        cell_idx,
                        cursor,
                        &selection,
                        glyph_color,
                        bg_color,
                        palette,
                    );

                    let cell_rect = Rect::new(
                        Point::new(
                            cell_idx as isize * self.render_metrics.cell_size.width,
                            self.render_metrics.cell_size.height * line_idx as isize,
                        ),
                        self.render_metrics.cell_size,
                    );
                    ctx.clear_rect(cell_rect, bg_color);

                    match underline {
                        Underline::Single => {
                            let software = self.render_state.software();
                            let sprite = &software.util_sprites.single_underline;
                            ctx.draw_image(
                                cell_rect.origin,
                                Some(sprite.coords),
                                &*sprite.texture.image.borrow(),
                                Operator::MultiplyThenOver(glyph_color),
                            );
                        }
                        Underline::Double => {
                            let software = self.render_state.software();
                            let sprite = &software.util_sprites.double_underline;
                            ctx.draw_image(
                                cell_rect.origin,
                                Some(sprite.coords),
                                &*sprite.texture.image.borrow(),
                                Operator::MultiplyThenOver(glyph_color),
                            );
                        }
                        Underline::None => {}
                    }
                    if attrs.strikethrough() {
                        let software = self.render_state.software();
                        let sprite = &software.util_sprites.strike_through;
                        ctx.draw_image(
                            cell_rect.origin,
                            Some(sprite.coords),
                            &*sprite.texture.image.borrow(),
                            Operator::MultiplyThenOver(glyph_color),
                        );
                    }

                    if let Some(ref texture) = glyph.texture {
                        let slice = SpriteSlice {
                            cell_idx: glyph_idx,
                            num_cells: info.num_cells as usize,
                            cell_width: self.render_metrics.cell_size.width as usize,
                            scale: glyph.scale as f32,
                            left_offset: left,
                        };
                        let left = if glyph_idx == 0 { left } else { 0.0 };

                        ctx.draw_image(
                            Point::new(
                                (cell_rect.origin.x as f32 + left) as isize,
                                (cell_rect.origin.y as f32 + top) as isize,
                            ),
                            Some(slice.pixel_rect(texture)),
                            &*texture.texture.image.borrow(),
                            if glyph.has_color {
                                // For full color glyphs, always use their color.
                                // This avoids rendering a black mask when the text
                                // selection moves over the glyph
                                Operator::Over
                            } else {
                                Operator::MultiplyThenOver(glyph_color)
                            },
                        );
                    } else if let Some(image) = attrs.image.as_ref() {
                        // Render iTerm2 style image attributes
                        let software = self.render_state.software();
                        if let Ok(sprite) = software
                            .glyph_cache
                            .borrow_mut()
                            .cached_image(image.image_data())
                        {
                            let width = sprite.coords.size.width;
                            let height = sprite.coords.size.height;

                            let top_left = image.top_left();
                            let bottom_right = image.bottom_right();
                            let origin = Point::new(
                                sprite.coords.origin.x + (*top_left.x * width as f32) as isize,
                                sprite.coords.origin.y + (*top_left.y * height as f32) as isize,
                            );

                            let coords = Rect::new(
                                origin,
                                Size::new(
                                    ((*bottom_right.x - *top_left.x) * width as f32) as isize,
                                    ((*bottom_right.y - *top_left.y) * height as f32) as isize,
                                ),
                            );

                            ctx.draw_image(
                                cell_rect.origin,
                                Some(coords),
                                &*sprite.texture.image.borrow(),
                                Operator::Over,
                            );
                        }
                    }
                }
            }
        }

        // Clear any remaining cells to the right of the clusters we
        // found above, otherwise we leave artifacts behind.  The easiest
        // reproduction for the artifacts is to maximize the window and
        // open a vim split horizontally.  Backgrounding vim would leave
        // the right pane with its prior contents instead of showing the
        // cleared lines from the shell in the main screen.

        for cell_idx in last_cell_idx + 1..num_cols {
            // Even though we don't have a cell for these, they still
            // hold the cursor or the selection so we need to compute
            // the colors in the usual way.
            let (_glyph_color, bg_color) = self.compute_cell_fg_bg(
                line_idx,
                cell_idx,
                cursor,
                &selection,
                rgbcolor_to_window_color(palette.foreground),
                rgbcolor_to_window_color(palette.background),
                palette,
            );

            let cell_rect = Rect::new(
                Point::new(
                    cell_idx as isize * self.render_metrics.cell_size.width,
                    self.render_metrics.cell_size.height * line_idx as isize,
                ),
                self.render_metrics.cell_size,
            );
            ctx.clear_rect(cell_rect, bg_color);
        }

        // Fill any marginal area to the right of the last cell
        let pixel_width_of_cells = num_cols * self.render_metrics.cell_size.width as usize;
        ctx.clear_rect(
            Rect::new(
                Point::new(
                    pixel_width_of_cells as isize,
                    self.render_metrics.cell_size.height * line_idx as isize,
                ),
                Size::new(
                    (self.dimensions.pixel_width - pixel_width_of_cells) as isize,
                    self.render_metrics.cell_size.height,
                ),
            ),
            rgbcolor_to_window_color(palette.background),
        );

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn compute_cell_fg_bg(
        &self,
        line_idx: usize,
        cell_idx: usize,
        cursor: &CursorPosition,
        selection: &Range<usize>,
        fg_color: Color,
        bg_color: Color,
        palette: &ColorPalette,
    ) -> (Color, Color) {
        let selected = selection.contains(&cell_idx);
        let is_cursor = line_idx as i64 == cursor.y && cursor.x == cell_idx;

        let (fg_color, bg_color) = match (selected, is_cursor) {
            // Normally, render the cell as configured
            (false, false) => (fg_color, bg_color),
            // Cursor cell overrides colors
            (_, true) => (
                rgbcolor_to_window_color(palette.cursor_fg),
                rgbcolor_to_window_color(palette.cursor_bg),
            ),
            // Selected text overrides colors
            (true, false) => (
                rgbcolor_to_window_color(palette.selection_fg),
                rgbcolor_to_window_color(palette.selection_bg),
            ),
        };

        (fg_color, bg_color)
    }
}

fn rgbcolor_to_window_color(color: RgbColor) -> Color {
    Color::rgba(color.red, color.green, color.blue, 0xff)
}

fn window_mods_to_termwiz_mods(modifiers: ::window::Modifiers) -> termwiz::input::Modifiers {
    let mut result = termwiz::input::Modifiers::NONE;
    if modifiers.contains(::window::Modifiers::SHIFT) {
        result.insert(termwiz::input::Modifiers::SHIFT);
    }
    if modifiers.contains(::window::Modifiers::ALT) {
        result.insert(termwiz::input::Modifiers::ALT);
    }
    if modifiers.contains(::window::Modifiers::CTRL) {
        result.insert(termwiz::input::Modifiers::CTRL);
    }
    if modifiers.contains(::window::Modifiers::SUPER) {
        result.insert(termwiz::input::Modifiers::SUPER);
    }
    result
}
