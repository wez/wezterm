#![cfg_attr(feature = "cargo-clippy", allow(clippy::range_plus_one))]
use super::quad::*;
use super::renderstate::*;
use super::utilsprites::RenderMetrics;
use crate::config::{configuration, ConfigHandle, TextStyle};
use crate::font::shaper::GlyphInfo;
use crate::font::units::*;
use crate::font::FontConfiguration;
use crate::frontend::activity::Activity;
use crate::frontend::front_end;
use crate::frontend::gui::overlay::{launcher, start_overlay, tab_navigator};
use crate::frontend::gui::scrollbar::*;
use crate::frontend::gui::selection::*;
use crate::frontend::gui::tabbar::{TabBarItem, TabBarState};
use crate::keyassignment::{KeyAssignment, KeyMap, SpawnCommand, SpawnTabDomain};
use crate::mux::domain::{DomainId, DomainState};
use crate::mux::renderable::{Renderable, RenderableDimensions, StableCursorPosition};
use crate::mux::tab::{Tab, TabId};
use crate::mux::window::WindowId as MuxWindowId;
use crate::mux::Mux;
use ::term::input::MouseButton as TMB;
use ::term::input::MouseEventKind as TMEK;
use ::window::bitmaps::atlas::{OutOfTextureSpace, SpriteSlice};
use ::window::bitmaps::Texture2d;
use ::window::glium::uniforms::{
    MagnifySamplerFilter, MinifySamplerFilter, Sampler, SamplerWrapFunction,
};
use ::window::glium::{uniform, BlendingFunction, LinearBlendingFactor, Surface};
use ::window::MouseButtons as WMB;
use ::window::MouseEventKind as WMEK;
use ::window::*;
use anyhow::{anyhow, bail, ensure};
use lru::LruCache;
use portable_pty::{CommandBuilder, PtySize};
use std::any::Any;
use std::cell::{RefCell, RefMut};
use std::collections::HashMap;
use std::convert::TryInto;
use std::ops::Range;
use std::ops::{Add, Sub};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use term::color::ColorPalette;
use term::input::LastMouseClick;
use term::{Line, StableRowIndex, Underline};
use termwiz::color::RgbColor;
use termwiz::hyperlink::Hyperlink;
use termwiz::surface::CursorShape;

#[derive(Debug, Clone, Copy)]
struct RowsAndCols {
    rows: usize,
    cols: usize,
}

/// ClipboardHelper bridges between the window crate clipboard
/// manipulation and the term crate clipboard interface
#[derive(Clone)]
pub struct ClipboardHelper {
    window: Window,
    clipboard_contents: Arc<Mutex<Option<String>>>,
}

impl term::Clipboard for ClipboardHelper {
    fn get_contents(&self) -> anyhow::Result<String> {
        // Even though we could request the clipboard contents using a call
        // like `self.window.get_clipboard().wait()` here, that requires
        // that the event loop be processed to do its work.
        // Since we are typically called in a blocking fashion on the
        // event loop, we have to manually arrange to populate the
        // clipboard_contents cache prior to calling the code that
        // might call us.
        Ok(self
            .clipboard_contents
            .lock()
            .unwrap()
            .as_ref()
            .cloned()
            .unwrap_or_else(String::new))
    }

    fn set_contents(&self, data: Option<String>) -> anyhow::Result<()> {
        self.window.set_clipboard(data.unwrap_or_else(String::new));
        Ok(())
    }
}

struct PrevCursorPos {
    pos: StableCursorPosition,
    when: Instant,
}

impl PrevCursorPos {
    fn new() -> Self {
        PrevCursorPos {
            pos: StableCursorPosition::default(),
            when: Instant::now(),
        }
    }

    /// Make the cursor look like it moved
    fn bump(&mut self) {
        self.when = Instant::now();
    }

    /// Update the cursor position if its different
    fn update(&mut self, newpos: &StableCursorPosition) {
        if &self.pos != newpos {
            self.pos = *newpos;
            self.when = Instant::now();
        }
    }

    /// When did the cursor last move?
    fn last_cursor_movement(&self) -> Instant {
        self.when
    }
}

#[derive(Default)]
struct TabState {
    /// If is_some(), the top row of the visible screen.
    /// Otherwise, the viewport is at the bottom of the
    /// scrollback.
    viewport: Option<StableRowIndex>,
    selection: Selection,
    /// If is_some(), rather than display the actual tab
    /// contents, we're overlaying a little internal application
    /// tab.  We'll also route input to it.
    overlay: Option<Rc<dyn Tab>>,
}

#[derive(PartialEq, Eq, Hash)]
struct ShapeCacheKey((TextStyle, String));

pub struct TermWindow {
    pub window: Option<Window>,
    /// When we most recently received keyboard focus
    focused: Option<Instant>,
    fonts: Rc<FontConfiguration>,
    /// Window dimensions and dpi
    dimensions: Dimensions,
    /// Terminal dimensions
    terminal_size: PtySize,
    mux_window_id: MuxWindowId,
    render_metrics: RenderMetrics,
    render_state: RenderState,
    keys: KeyMap,
    show_tab_bar: bool,
    show_scroll_bar: bool,
    tab_bar: TabBarState,
    last_mouse_coords: (usize, i64),
    scroll_drag_start: Option<isize>,
    config_generation: usize,
    prev_cursor: PrevCursorPos,
    last_scroll_info: RenderableDimensions,

    tab_state: RefCell<HashMap<TabId, TabState>>,

    /// Gross workaround for managing async keyboard fetching
    /// just for middle mouse button paste function
    clipboard_contents: Arc<Mutex<Option<String>>>,

    current_mouse_button: Option<MousePress>,

    /// Keeps track of double and triple clicks
    last_mouse_click: Option<LastMouseClick>,

    /// The URL over which we are currently hovering
    current_highlight: Option<Arc<Hyperlink>>,

    shape_cache: RefCell<LruCache<ShapeCacheKey, anyhow::Result<Vec<GlyphInfo>>>>,
}

struct Host<'a> {
    writer: &'a mut dyn std::io::Write,
}

impl<'a> term::TerminalHost for Host<'a> {
    fn writer(&mut self) -> &mut dyn std::io::Write {
        self.writer
    }
}

enum Key {
    Code(::termwiz::input::KeyCode),
    Composed(String),
    None,
}

impl WindowCallbacks for TermWindow {
    fn created(&mut self, window: &Window) {
        self.window.replace(window.clone());
    }

    fn can_close(&mut self) -> bool {
        let mux = Mux::get().unwrap();
        let tab_ids: Vec<TabId> = if let Some(win) = mux.get_window(self.mux_window_id) {
            win.iter().map(|tab| tab.tab_id()).collect()
        } else {
            return true;
        };

        for id in tab_ids {
            mux.remove_tab(id);
        }
        true
    }

    fn as_any(&mut self) -> &mut dyn Any {
        self
    }

    fn focus_change(&mut self, focused: bool) {
        log::trace!("Setting focus to {:?}", focused);
        self.focused = if focused { Some(Instant::now()) } else { None };

        if self.focused.is_none() {
            self.last_mouse_click = None;
            self.current_mouse_button = None;
        }

        // Reset the cursor blink phase
        self.prev_cursor.bump();

        // force cursor to be repainted
        self.window.as_ref().unwrap().invalidate();
    }

    fn mouse_event(&mut self, event: &MouseEvent, context: &dyn WindowOps) {
        let tab = match self.get_active_tab_or_overlay() {
            Some(tab) => tab,
            None => return,
        };

        let config = configuration();
        let x = (event
            .coords
            .x
            .sub(config.window_padding.left as isize)
            .max(0)
            / self.render_metrics.cell_size.width) as usize;
        let y = (event
            .coords
            .y
            .sub(config.window_padding.top as isize)
            .max(0)
            / self.render_metrics.cell_size.height) as i64;

        let first_line_offset = if self.show_tab_bar { 1 } else { 0 };
        self.last_mouse_coords = (x, y);

        let in_tab_bar = self.show_tab_bar && y == 0 && event.coords.y >= 0;
        let in_scroll_bar = self.show_scroll_bar && x >= self.terminal_size.cols as usize;
        // y position relative to top of viewport (not including tab bar)
        let term_y = y.saturating_sub(first_line_offset);

        match event.kind {
            WMEK::Release(ref press) => {
                self.current_mouse_button = None;
                if press == &MousePress::Left && self.scroll_drag_start.take().is_some() {
                    // Completed a drag
                    return;
                }
            }

            WMEK::Press(ref press) => {
                if let Some(focused) = self.focused.as_ref() {
                    if focused.elapsed() <= Duration::from_millis(200) {
                        log::trace!("discard mouse click because it focused the window");
                        return;
                    }
                }

                // Perform click counting
                let button = match press {
                    MousePress::Left => TMB::Left,
                    MousePress::Middle => TMB::Middle,
                    MousePress::Right => TMB::Right,
                };

                let click = match self.last_mouse_click.take() {
                    None => LastMouseClick::new(button),
                    Some(click) => click.add(button),
                };
                self.last_mouse_click = Some(click);
                self.current_mouse_button = Some(press.clone());
            }

            WMEK::VertWheel(amount) if !tab.is_mouse_grabbed() => {
                // adjust viewport
                let render = tab.renderer();
                let dims = render.get_dimensions();
                let position = self
                    .get_viewport(tab.tab_id())
                    .unwrap_or(dims.physical_top)
                    .saturating_sub(amount.into());
                self.set_viewport(tab.tab_id(), Some(position), dims);
                context.invalidate();
                return;
            }

            WMEK::Move => {
                let current_viewport = self.get_viewport(tab.tab_id());
                if let Some(from_top) = self.scroll_drag_start.as_ref() {
                    // Dragging the scroll bar
                    let tab = match self.get_active_tab_or_overlay() {
                        Some(tab) => tab,
                        None => return,
                    };

                    let render = tab.renderer();
                    let dims = render.get_dimensions();

                    let effective_thumb_top =
                        event.coords.y.saturating_sub(*from_top).max(0) as usize;

                    // Convert thumb top into a row index by reversing the math
                    // in ScrollHit::thumb
                    let row = ScrollHit::thumb_top_to_scroll_top(
                        effective_thumb_top,
                        &*render,
                        current_viewport,
                        self.terminal_size,
                        &self.dimensions,
                    );
                    self.set_viewport(tab.tab_id(), Some(row), dims);
                    context.invalidate();
                    return;
                }
            }
            _ => {}
        }

        if in_tab_bar {
            self.mouse_event_tab_bar(x, event, context);
        } else if in_scroll_bar {
            self.mouse_event_scroll_bar(tab, event, context);
        } else {
            self.mouse_event_terminal(tab, x, term_y, event, context);
        }
    }

    fn resize(&mut self, dimensions: Dimensions) {
        log::trace!(
            "resize event, current cells: {:?}, new dims: {:?}",
            self.current_cell_dimensions(),
            dimensions
        );
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

        // log::error!("key_event {:?}", key);

        let tab = match self.get_active_tab_or_overlay() {
            Some(tab) => tab,
            None => return false,
        };
        let modifiers = window_mods_to_termwiz_mods(key.modifiers);

        // First chance to operate on the raw key; if it matches a
        // user-defined key binding then we execute it and stop there.
        if let Some(key) = &key.raw_key {
            if let Key::Code(key) = self.win_key_code_to_termwiz_key_code(&key) {
                if let Some(assignment) = self.keys.lookup(key, modifiers) {
                    self.perform_key_assignment(&tab, &assignment).ok();
                    return true;
                }

                if !configuration().send_composed_key_when_alt_is_pressed
                    && modifiers.contains(::termwiz::input::Modifiers::ALT)
                    && tab.key_down(key, modifiers).is_ok()
                {
                    if !key.is_modifier() {
                        self.maybe_scroll_to_bottom_for_input(&tab);
                    }
                    return true;
                }
            }
        }

        let key = self.win_key_code_to_termwiz_key_code(&key.key);
        match key {
            Key::Code(key) => {
                if let Some(assignment) = self.keys.lookup(key, modifiers) {
                    self.perform_key_assignment(&tab, &assignment).ok();
                    true
                } else if tab.key_down(key, modifiers).is_ok() {
                    if !key.is_modifier() {
                        self.maybe_scroll_to_bottom_for_input(&tab);
                    }
                    true
                } else {
                    false
                }
            }
            Key::Composed(s) => {
                tab.writer().write_all(s.as_bytes()).ok();
                self.maybe_scroll_to_bottom_for_input(&tab);
                true
            }
            Key::None => false,
        }
    }

    fn paint(&mut self, ctx: &mut dyn PaintContext) {
        let tab = match self.get_active_tab_or_overlay() {
            Some(tab) => tab,
            None => {
                ctx.clear(Color::rgb(0, 0, 0));
                return;
            }
        };

        self.check_for_config_reload();
        self.update_text_cursor(&tab);
        self.update_title();

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
                // Recursively initiate a new paint
                return self.paint(ctx);
            }
            log::error!("paint failed: {}", err);
        }
        log::debug!("paint_tab elapsed={:?}", start.elapsed());
        metrics::value!("gui.paint.software", start.elapsed());
    }

    fn paint_opengl(&mut self, frame: &mut glium::Frame) {
        let tab = match self.get_active_tab_or_overlay() {
            Some(tab) => tab,
            None => {
                frame.clear_color_srgb(0., 0., 0., 1.);
                return;
            }
        };
        self.check_for_config_reload();
        self.update_text_cursor(&tab);
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
                // Recursively initiate a new paint
                return self.paint_opengl(frame);
            }
            log::error!("paint_tab_opengl failed: {}", err);
        }
        log::debug!("paint_tab_opengl elapsed={:?}", start.elapsed());
        metrics::value!("gui.paint.opengl", start.elapsed());
        self.update_title();
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

impl TermWindow {
    pub fn new_window(
        config: &ConfigHandle,
        fontconfig: &Rc<FontConfiguration>,
        tab: &Rc<dyn Tab>,
        mux_window_id: MuxWindowId,
    ) -> anyhow::Result<()> {
        let dims = tab.renderer().get_dimensions();
        let physical_rows = dims.viewport_rows;
        let physical_cols = dims.cols;

        let render_metrics = RenderMetrics::new(fontconfig);

        let terminal_size = PtySize {
            rows: physical_rows as u16,
            cols: physical_cols as u16,
            pixel_width: (render_metrics.cell_size.width as usize * physical_cols) as u16,
            pixel_height: (render_metrics.cell_size.height as usize * physical_rows) as u16,
        };

        // Initially we have only a single tab, so take that into account
        // for the tab bar state.
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
            dpi: config.dpi as usize,
        };

        log::info!(
            "TermWindow::new_window called with mux_window_id {} {:?} {:?}",
            mux_window_id,
            terminal_size,
            dimensions
        );

        const ATLAS_SIZE: usize = 4096;
        let render_state = RenderState::Software(SoftwareRenderState::new(
            fontconfig,
            &render_metrics,
            ATLAS_SIZE,
        )?);

        let clipboard_contents = Arc::new(Mutex::new(None));

        let window = Window::new_window(
            "wezterm",
            "wezterm",
            dimensions.pixel_width,
            dimensions.pixel_height,
            Box::new(Self {
                window: None,
                focused: None,
                mux_window_id,
                fonts: Rc::clone(fontconfig),
                render_metrics,
                dimensions,
                terminal_size,
                render_state,
                keys: KeyMap::new(),
                show_tab_bar,
                show_scroll_bar: config.enable_scroll_bar,
                tab_bar: TabBarState::default(),
                last_mouse_coords: (0, -1),
                scroll_drag_start: None,
                config_generation: config.generation(),
                prev_cursor: PrevCursorPos::new(),
                last_scroll_info: RenderableDimensions::default(),
                clipboard_contents: Arc::clone(&clipboard_contents),
                tab_state: RefCell::new(HashMap::new()),
                current_mouse_button: None,
                last_mouse_click: None,
                current_highlight: None,
                shape_cache: RefCell::new(LruCache::new(65536)),
            }),
        )?;

        let cloned_window = window.clone();

        Connection::get()
            .unwrap()
            .schedule_timer(std::time::Duration::from_millis(35), {
                let mut last_blink_paint = Instant::now();
                move || {
                    cloned_window.apply(move |myself, _| {
                        if let Some(myself) = myself.downcast_mut::<Self>() {
                            let mux = Mux::get().unwrap();

                            if let Some(tab) = myself.get_active_tab_or_overlay() {
                                // If the config was reloaded, ask the window to apply
                                // and render any changes
                                myself.check_for_config_reload();

                                let config = configuration();

                                let render = tab.renderer();

                                // If blinking is permitted, and the cursor shape is set
                                // to a blinking variant, and it's been longer than the
                                // blink rate interval, then invalid the lines in the terminal
                                // so that we will re-evaluate the cursor visibility.
                                // This is pretty heavyweight: it would be nice to only invalidate
                                // the line on which the cursor resides, and then only if the cursor
                                // is within the viewport.
                                if config.cursor_blink_rate != 0 {
                                    let shape = config
                                        .default_cursor_style
                                        .effective_shape(render.get_cursor_position().shape);
                                    if shape.is_blinking() {
                                        let now = Instant::now();
                                        if now.duration_since(last_blink_paint)
                                            > Duration::from_millis(config.cursor_blink_rate)
                                        {
                                            myself.window.as_ref().unwrap().invalidate();
                                            last_blink_paint = now;
                                        }
                                    }
                                }

                                // If the model is dirty, arrange to re-paint
                                let dims = render.get_dimensions();
                                let viewport = myself
                                    .get_viewport(tab.tab_id())
                                    .unwrap_or(dims.physical_top);
                                let visible_range =
                                    viewport..viewport + dims.viewport_rows as StableRowIndex;
                                let dirty = render.get_dirty_lines(visible_range);

                                if !dirty.is_empty() {
                                    // If any of the changed lines intersect with the selection,
                                    // then we need to clear the selection
                                    let clear_selection = if let Some(selection_range) =
                                        myself.selection(tab.tab_id()).range.as_ref()
                                    {
                                        let selection_rows = selection_range.rows();
                                        selection_rows.into_iter().any(|row| dirty.contains(row))
                                    } else {
                                        false
                                    };

                                    if clear_selection {
                                        myself.selection(tab.tab_id()).range.take();
                                        myself.selection(tab.tab_id()).start.take();
                                    }

                                    myself.window.as_ref().unwrap().invalidate();
                                }

                                if let Some(mut mux_window) = mux.get_window_mut(mux_window_id) {
                                    if mux_window.check_and_reset_invalidated() {
                                        myself.window.as_ref().unwrap().invalidate();
                                    }
                                }
                            } else {
                                myself.window.as_ref().unwrap().close();
                            }
                        }
                        Ok(())
                    });
                }
            });

        let clipboard: Arc<dyn term::Clipboard> = Arc::new(ClipboardHelper {
            window: window.clone(),
            clipboard_contents,
        });
        tab.set_clipboard(&clipboard);
        Mux::get()
            .unwrap()
            .get_window_mut(mux_window_id)
            .unwrap()
            .set_clipboard(&clipboard);

        if super::is_opengl_enabled() {
            window.enable_opengl(|any, window, maybe_ctx| {
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
                                log::info!(
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
                };

                window.show();
                Ok(())
            });
        } else {
            window.show();
        }

        Ok(())
    }

    fn win_key_code_to_termwiz_key_code(&self, key: &::window::KeyCode) -> Key {
        use ::termwiz::input::KeyCode as KC;
        use ::window::KeyCode as WK;

        let code = match key {
            // TODO: consider eliminating these codes from termwiz::input::KeyCode
            WK::Char('\r') => KC::Enter,
            WK::Char('\t') => KC::Tab,
            WK::Char('\u{08}') => {
                if configuration().swap_backspace_and_delete {
                    KC::Delete
                } else {
                    KC::Backspace
                }
            }
            WK::Char('\u{7f}') => {
                if configuration().swap_backspace_and_delete {
                    KC::Backspace
                } else {
                    KC::Delete
                }
            }
            WK::Char('\u{1b}') => KC::Escape,

            WK::Char(c) => KC::Char(*c),
            WK::Composed(ref s) => {
                let mut chars = s.chars();
                if let Some(first_char) = chars.next() {
                    if chars.next().is_none() {
                        // Was just a single char after all
                        return self.win_key_code_to_termwiz_key_code(&WK::Char(first_char));
                    }
                }
                return Key::Composed(s.to_owned());
            }
            WK::Function(f) => KC::Function(*f),
            WK::LeftArrow => KC::LeftArrow,
            WK::RightArrow => KC::RightArrow,
            WK::UpArrow => KC::UpArrow,
            WK::DownArrow => KC::DownArrow,
            WK::Home => KC::Home,
            WK::End => KC::End,
            WK::PageUp => KC::PageUp,
            WK::PageDown => KC::PageDown,
            WK::Insert => KC::Insert,
            WK::Hyper => KC::Hyper,
            WK::Super => KC::Super,
            WK::Meta => KC::Meta,
            WK::Cancel => KC::Cancel,
            WK::Clear => KC::Clear,
            WK::Shift => KC::Shift,
            WK::LeftShift => KC::LeftShift,
            WK::RightShift => KC::RightShift,
            WK::Control => KC::Control,
            WK::LeftControl => KC::LeftControl,
            WK::RightControl => KC::RightControl,
            WK::Alt => KC::Alt,
            WK::LeftAlt => KC::LeftAlt,
            WK::RightAlt => KC::RightAlt,
            WK::Pause => KC::Pause,
            WK::CapsLock => KC::CapsLock,
            WK::Select => KC::Select,
            WK::Print => KC::Print,
            WK::Execute => KC::Execute,
            WK::PrintScreen => KC::PrintScreen,
            WK::Help => KC::Help,
            WK::LeftWindows => KC::LeftWindows,
            WK::RightWindows => KC::RightWindows,
            WK::Sleep => KC::Sleep,
            WK::Multiply => KC::Multiply,
            WK::Applications => KC::Applications,
            WK::Add => KC::Add,
            WK::Numpad(0) => KC::Numpad0,
            WK::Numpad(1) => KC::Numpad1,
            WK::Numpad(2) => KC::Numpad2,
            WK::Numpad(3) => KC::Numpad3,
            WK::Numpad(4) => KC::Numpad4,
            WK::Numpad(5) => KC::Numpad5,
            WK::Numpad(6) => KC::Numpad6,
            WK::Numpad(7) => KC::Numpad7,
            WK::Numpad(8) => KC::Numpad8,
            WK::Numpad(9) => KC::Numpad9,
            WK::Numpad(_) => return Key::None,
            WK::Separator => KC::Separator,
            WK::Subtract => KC::Subtract,
            WK::Decimal => KC::Decimal,
            WK::Divide => KC::Divide,
            WK::NumLock => KC::NumLock,
            WK::ScrollLock => KC::ScrollLock,
            WK::BrowserBack => KC::BrowserBack,
            WK::BrowserForward => KC::BrowserForward,
            WK::BrowserRefresh => KC::BrowserRefresh,
            WK::BrowserStop => KC::BrowserStop,
            WK::BrowserSearch => KC::BrowserSearch,
            WK::BrowserFavorites => KC::BrowserFavorites,
            WK::BrowserHome => KC::BrowserHome,
            WK::VolumeMute => KC::VolumeMute,
            WK::VolumeDown => KC::VolumeDown,
            WK::VolumeUp => KC::VolumeUp,
            WK::MediaNextTrack => KC::MediaNextTrack,
            WK::MediaPrevTrack => KC::MediaPrevTrack,
            WK::MediaStop => KC::MediaStop,
            WK::MediaPlayPause => KC::MediaPlayPause,
            WK::ApplicationLeftArrow => KC::ApplicationLeftArrow,
            WK::ApplicationRightArrow => KC::ApplicationRightArrow,
            WK::ApplicationUpArrow => KC::ApplicationUpArrow,
            WK::ApplicationDownArrow => KC::ApplicationDownArrow,
        };
        Key::Code(code)
    }

    fn recreate_texture_atlas(&mut self, size: Option<usize>) -> anyhow::Result<()> {
        self.render_state
            .recreate_texture_atlas(&self.fonts, &self.render_metrics, size)
    }

    fn check_for_config_reload(&mut self) {
        if self.config_generation != configuration().generation() {
            self.config_was_reloaded();
        }
    }

    fn config_was_reloaded(&mut self) {
        let config = configuration();
        self.config_generation = config.generation();

        let mux = Mux::get().unwrap();
        let window = match mux.get_window(self.mux_window_id) {
            Some(window) => window,
            _ => return,
        };
        self.show_tab_bar =
            config.enable_tab_bar && (window.len() > 1) || !config.hide_tab_bar_if_only_one_tab;

        self.show_scroll_bar = config.enable_scroll_bar;
        self.shape_cache.borrow_mut().clear();
        self.keys = KeyMap::new();
        let dimensions = self.dimensions;
        let cell_dims = self.current_cell_dimensions();
        self.apply_scale_change(&dimensions, self.fonts.get_font_scale());
        self.apply_dimensions(&dimensions, Some(cell_dims));
        if let Some(window) = self.window.as_ref() {
            window.invalidate();
        }
    }

    fn update_scrollbar(&mut self) {
        if !self.show_scroll_bar {
            return;
        }

        let tab = match self.get_active_tab_or_overlay() {
            Some(tab) => tab,
            None => return,
        };

        let render_dims = tab.renderer().get_dimensions();
        if render_dims == self.last_scroll_info {
            return;
        }

        self.last_scroll_info = render_dims;

        if let Some(window) = self.window.as_ref() {
            window.invalidate();
        }
    }

    fn update_title(&mut self) {
        let mux = Mux::get().unwrap();
        let window = match mux.get_window(self.mux_window_id) {
            Some(window) => window,
            _ => return,
        };
        let config = configuration();

        let new_tab_bar = TabBarState::new(
            self.terminal_size.cols as usize,
            if self.last_mouse_coords.1 == 0 {
                Some(self.last_mouse_coords.0)
            } else {
                None
            },
            &window,
            config.colors.as_ref().and_then(|c| c.tab_bar.as_ref()),
        );
        if new_tab_bar != self.tab_bar {
            self.tab_bar = new_tab_bar;
            if let Some(window) = self.window.as_ref() {
                window.invalidate();
            }
        }

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
            let show_tab_bar;
            if num_tabs == 1 {
                window.set_title(&title);
                show_tab_bar = config.enable_tab_bar && !config.hide_tab_bar_if_only_one_tab;
            } else {
                window.set_title(&format!("[{}/{}] {}", tab_no + 1, num_tabs, title));
                show_tab_bar = config.enable_tab_bar;
            }

            // If the number of tabs changed and caused the tab bar to
            // hide/show, then we'll need to resize things.  It is simplest
            // to piggy back on the config reloading code for that, so that
            // is what we're doing.
            if show_tab_bar != self.show_tab_bar {
                self.config_was_reloaded();
            }
        }
    }

    fn update_text_cursor(&mut self, tab: &Rc<dyn Tab>) {
        let term = tab.renderer();
        let cursor = term.get_cursor_position();
        if let Some(win) = self.window.as_ref() {
            let config = configuration();
            let top = term.get_dimensions().physical_top + if self.show_tab_bar { -1 } else { 0 };
            let r = Rect::new(
                Point::new(
                    (cursor.x.max(0) as isize * self.render_metrics.cell_size.width)
                        .add(config.window_padding.left as isize),
                    ((cursor.y - top).max(0) as isize * self.render_metrics.cell_size.height)
                        .add(config.window_padding.top as isize),
                ),
                self.render_metrics.cell_size,
            );
            win.set_text_cursor_position(r);
        }
    }

    fn activate_tab(&mut self, tab_idx: usize) -> anyhow::Result<()> {
        let mux = Mux::get().unwrap();
        let mut window = mux
            .get_window_mut(self.mux_window_id)
            .ok_or_else(|| anyhow!("no such window"))?;

        let max = window.len();
        if tab_idx < max {
            window.set_active(tab_idx);

            drop(window);
            self.update_title();
            self.update_scrollbar();
        }
        Ok(())
    }

    fn activate_tab_relative(&mut self, delta: isize) -> anyhow::Result<()> {
        let mux = Mux::get().unwrap();
        let window = mux
            .get_window(self.mux_window_id)
            .ok_or_else(|| anyhow!("no such window"))?;

        let max = window.len();
        ensure!(max > 0, "no more tabs");

        let active = window.get_active_idx() as isize;
        let tab = active + delta;
        let tab = if tab < 0 { max as isize + tab } else { tab };
        drop(window);
        self.activate_tab(tab as usize % max)
    }

    fn move_tab(&mut self, tab_idx: usize) -> anyhow::Result<()> {
        let mux = Mux::get().unwrap();
        let mut window = mux
            .get_window_mut(self.mux_window_id)
            .ok_or_else(|| anyhow!("no such window"))?;

        let max = window.len();
        ensure!(max > 0, "no more tabs");

        let active = window.get_active_idx();

        ensure!(tab_idx < max, "cannot move a tab out of range");

        let tab_inst = window.remove_by_idx(active);
        window.insert(tab_idx, &tab_inst);
        window.set_active(tab_idx);

        drop(window);
        self.update_title();
        self.update_scrollbar();

        Ok(())
    }

    fn show_tab_navigator(&mut self) {
        let mux = Mux::get().unwrap();
        let tab = match mux.get_active_tab_for_window(self.mux_window_id) {
            Some(tab) => tab,
            None => return,
        };

        let window = mux
            .get_window(self.mux_window_id)
            .expect("to resolve my own window_id");

        // Ideally we'd resolve the tabs on the fly once we've started the
        // overlay, but since the overlay runs in a different thread, accessing
        // the mux list is a bit awkward.  To get the ball rolling we capture
        // the list of tabs up front and live with a static list.
        let tabs: Vec<(String, TabId)> = window
            .iter()
            .map(|tab| (tab.get_title(), tab.tab_id()))
            .collect();

        let mux_window_id = self.mux_window_id;
        let (overlay, future) = start_overlay(self, &tab, move |tab_id, term| {
            tab_navigator(tab_id, term, tabs, mux_window_id)
        });
        self.assign_overlay(tab.tab_id(), overlay);
        promise::spawn::spawn(future);
    }

    fn show_launcher(&mut self) {
        let mux = Mux::get().unwrap();
        let tab = match mux.get_active_tab_for_window(self.mux_window_id) {
            Some(tab) => tab,
            None => return,
        };

        let mux_window_id = self.mux_window_id;

        let clipboard = ClipboardHelper {
            window: self.window.as_ref().unwrap().clone(),
            clipboard_contents: Arc::clone(&self.clipboard_contents),
        };

        let mut domains = mux.iter_domains();
        domains.sort_by(|a, b| {
            let a_state = a.state();
            let b_state = b.state();
            if a_state != b_state {
                use std::cmp::Ordering;
                return if a_state == DomainState::Attached {
                    Ordering::Less
                } else {
                    Ordering::Greater
                };
            }
            a.domain_id().cmp(&b.domain_id())
        });
        domains.retain(|dom| dom.spawnable());
        let domains: Vec<(DomainId, DomainState, String)> = domains
            .iter()
            .map(|dom| {
                let name = dom.domain_name();
                let label = dom.domain_label();
                let label = if name == label || label == "" {
                    format!("domain `{}`", name)
                } else {
                    format!("domain `{}` - {}", name, label)
                };
                (dom.domain_id(), dom.state(), label)
            })
            .collect();

        let domain_id_of_current_tab = tab.domain_id();
        let size = self.terminal_size;

        let (overlay, future) = start_overlay(self, &tab, move |tab_id, term| {
            launcher(
                tab_id,
                domain_id_of_current_tab,
                term,
                mux_window_id,
                domains,
                clipboard,
                size,
            )
        });
        self.assign_overlay(tab.tab_id(), overlay);
        promise::spawn::spawn(future);
    }

    fn scroll_by_page(&mut self, amount: isize) -> anyhow::Result<()> {
        let tab = match self.get_active_tab_or_overlay() {
            Some(tab) => tab,
            None => return Ok(()),
        };
        let render = tab.renderer();
        let dims = render.get_dimensions();
        let position = self
            .get_viewport(tab.tab_id())
            .unwrap_or(dims.physical_top)
            .saturating_add(amount * dims.viewport_rows as isize);
        self.set_viewport(tab.tab_id(), Some(position), dims);
        if let Some(win) = self.window.as_ref() {
            win.invalidate();
        }
        Ok(())
    }

    fn move_tab_relative(&mut self, delta: isize) -> anyhow::Result<()> {
        let mux = Mux::get().unwrap();
        let window = mux
            .get_window(self.mux_window_id)
            .ok_or_else(|| anyhow!("no such window"))?;

        let max = window.len();
        ensure!(max > 0, "no more tabs");

        let active = window.get_active_idx();
        let tab = active as isize + delta;
        let tab = if tab < 0 {
            0usize
        } else if tab >= max as isize {
            max - 1
        } else {
            tab as usize
        };

        drop(window);
        self.move_tab(tab)
    }

    fn spawn_command(&mut self, spawn: &SpawnCommand, new_window: bool) {
        Self::spawn_command_impl(
            spawn,
            new_window,
            self.terminal_size,
            self.mux_window_id,
            ClipboardHelper {
                window: self.window.as_ref().unwrap().clone(),
                clipboard_contents: Arc::clone(&self.clipboard_contents),
            },
        )
    }

    pub fn spawn_command_impl(
        spawn: &SpawnCommand,
        new_window: bool,
        size: PtySize,
        mux_window_id: MuxWindowId,
        clipboard: ClipboardHelper,
    ) {
        let spawn = spawn.clone();

        promise::spawn::spawn(async move {
            let mux = Mux::get().unwrap();
            let activity = Activity::new();

            let mux_window_id = if new_window {
                mux.new_empty_window()
            } else {
                mux_window_id
            };

            let (domain, cwd) = match spawn.domain {
                SpawnTabDomain::DefaultDomain => {
                    let cwd = mux
                        .get_active_tab_for_window(mux_window_id)
                        .and_then(|tab| tab.get_current_working_dir());
                    (mux.default_domain().clone(), cwd)
                }
                SpawnTabDomain::CurrentTabDomain => {
                    if new_window {
                        // CurrentTabDomain is the default value for the spawn domain.
                        // It doesn't make sense to use it when spawning a new window,
                        // so we treat it as DefaultDomain instead.
                        let cwd = mux
                            .get_active_tab_for_window(mux_window_id)
                            .and_then(|tab| tab.get_current_working_dir());
                        (mux.default_domain().clone(), cwd)
                    } else {
                        let tab = match mux.get_active_tab_for_window(mux_window_id) {
                            Some(tab) => tab,
                            None => bail!("window has no tabs?"),
                        };
                        (
                            mux.get_domain(tab.domain_id()).ok_or_else(|| {
                                anyhow!("current tab has unresolvable domain id!?")
                            })?,
                            tab.get_current_working_dir(),
                        )
                    }
                }
                SpawnTabDomain::Domain(id) => (
                    mux.get_domain(id)
                        .ok_or_else(|| anyhow!("spawn_tab called with unresolvable domain id!?"))?,
                    None,
                ),
                SpawnTabDomain::DomainName(name) => (
                    mux.get_domain_by_name(&name).ok_or_else(|| {
                        anyhow!("spawn_tab called with unresolvable domain name {}", name)
                    })?,
                    None,
                ),
            };

            if domain.state() == DomainState::Detached {
                bail!("Cannot spawn a tab into a Detached domain");
            }

            let cwd = if let Some(cwd) = spawn.cwd.as_ref() {
                Some(cwd.to_str().map(|s| s.to_owned()).ok_or_else(|| {
                    anyhow!(
                        "Domain::spawn requires that the cwd be unicode in {:?}",
                        cwd
                    )
                })?)
            } else {
                match cwd {
                    Some(url) if url.scheme() == "file" => {
                        let path = url.path().to_string();
                        // On Windows the file URI can produce a path like:
                        // `/C:\Users` which is valid in a file URI, but the leading slash
                        // is not liked by the windows file APIs, so we strip it off here.
                        let bytes = path.as_bytes();
                        if bytes.len() > 2 && bytes[0] == b'/' && bytes[2] == b':' {
                            Some(path[1..].to_owned())
                        } else {
                            Some(path)
                        }
                    }
                    Some(_) | None => None,
                }
            };

            let cmd_builder = if let Some(args) = spawn.args {
                let mut builder = CommandBuilder::from_argv(args.iter().map(Into::into).collect());
                for (k, v) in spawn.set_environment_variables.iter() {
                    builder.env(k, v);
                }
                if let Some(cwd) = spawn.cwd {
                    builder.cwd(cwd);
                }
                Some(builder)
            } else {
                None
            };

            let tab = domain.spawn(size, cmd_builder, cwd, mux_window_id).await?;
            let tab_id = tab.tab_id();

            if new_window {
                let front_end = front_end().expect("to be called on gui thread");
                let fonts = Rc::new(FontConfiguration::new());
                front_end.spawn_new_window(&fonts, &tab, mux_window_id)?;
            } else {
                let clipboard: Arc<dyn term::Clipboard> = Arc::new(clipboard);
                tab.set_clipboard(&clipboard);
                let mut window = mux
                    .get_window_mut(mux_window_id)
                    .ok_or_else(|| anyhow!("no such window!?"))?;
                if let Some(idx) = window.idx_by_id(tab_id) {
                    window.set_active(idx);
                }
            }

            drop(activity);

            Ok(())
        });
    }

    fn spawn_tab(&mut self, domain: &SpawnTabDomain) {
        self.spawn_command(
            &SpawnCommand {
                domain: domain.clone(),
                ..Default::default()
            },
            false,
        );
    }

    fn selection_text(&self, tab: &Rc<dyn Tab>) -> String {
        let mut s = String::new();
        if let Some(sel) = self
            .selection(tab.tab_id())
            .range
            .as_ref()
            .map(|r| r.normalize())
        {
            let mut last_was_wrapped = false;
            let mut renderer = tab.renderer();
            let (first_row, lines) = renderer.get_lines(sel.rows());
            for (idx, line) in lines.iter().enumerate() {
                let cols = sel.cols_for_row(first_row + idx as StableRowIndex);
                let last_col_idx = cols.end.min(line.cells().len()).saturating_sub(1);
                if !s.is_empty() && !last_was_wrapped {
                    s.push('\n');
                }
                s.push_str(line.columns_as_str(cols).trim_end());

                let last_cell = &line.cells()[last_col_idx];
                // TODO: should really test for any unicode whitespace
                last_was_wrapped = last_cell.attrs().wrapped() && last_cell.str() != " ";
            }
        }

        s
    }

    fn perform_key_assignment(
        &mut self,
        tab: &Rc<dyn Tab>,
        assignment: &KeyAssignment,
    ) -> anyhow::Result<()> {
        use KeyAssignment::*;
        match assignment {
            SpawnTab(spawn_where) => {
                self.spawn_tab(spawn_where);
            }
            SpawnWindow => {
                self.spawn_new_window();
            }
            SpawnCommandInNewTab(spawn) => {
                self.spawn_command(spawn, false);
            }
            SpawnCommandInNewWindow(spawn) => {
                self.spawn_command(spawn, true);
            }
            ToggleFullScreen => {
                // self.toggle_full_screen(),
            }
            Copy => {
                self.window
                    .as_ref()
                    .unwrap()
                    .set_clipboard(self.selection_text(tab));
            }
            Paste => {
                let tab_id = tab.tab_id();
                let future = self.window.as_ref().unwrap().get_clipboard();
                promise::spawn::spawn(async move {
                    if let Ok(clip) = future.await {
                        let mux = Mux::get().unwrap();
                        if let Some(tab) = mux.get_tab(tab_id) {
                            tab.trickle_paste(clip).ok();
                        }
                    }
                });
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
            ReloadConfiguration => crate::config::reload(),
            MoveTab(n) => self.move_tab(*n)?,
            MoveTabRelative(n) => self.move_tab_relative(*n)?,
            ScrollByPage(n) => self.scroll_by_page(*n)?,
            ShowTabNavigator => self.show_tab_navigator(),
            ShowLauncher => self.show_launcher(),
            HideApplication => {
                let con = Connection::get().expect("call on gui thread");
                con.hide_application();
            }
            QuitApplication => {
                let con = Connection::get().expect("call on gui thread");
                con.terminate_message_loop();
            }
        };
        Ok(())
    }

    pub fn spawn_new_window(&mut self) {
        async fn new_window() -> anyhow::Result<()> {
            let mux = Mux::get().unwrap();
            let config = crate::config::configuration();
            let fonts = Rc::new(FontConfiguration::new());
            let window_id = mux.new_empty_window();
            let tab = mux
                .default_domain()
                .spawn(config.initial_size(), None, None, window_id)
                .await?;
            let front_end = front_end().expect("to be called on gui thread");
            front_end.spawn_new_window(&fonts, &tab, window_id)?;
            Ok::<(), anyhow::Error>(())
        }
        promise::spawn::spawn(async move {
            new_window().await.ok();
        });
    }

    fn apply_scale_change(&mut self, dimensions: &Dimensions, font_scale: f64) {
        self.fonts
            .change_scaling(font_scale, dimensions.dpi as f64 / 96.);
        self.render_metrics = RenderMetrics::new(&self.fonts);

        self.recreate_texture_atlas(None)
            .expect("failed to recreate atlas");
    }

    fn apply_dimensions(
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

        let config = configuration();

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
            let avail_width = dimensions.pixel_width
                - (config.window_padding.left + self.effective_right_padding(&config)) as usize;
            let avail_height = dimensions.pixel_height
                - (config.window_padding.top + config.window_padding.bottom) as usize;

            let rows = (avail_height / self.render_metrics.cell_size.height as usize)
                .saturating_sub(if self.show_tab_bar { 1 } else { 0 });
            let cols = avail_width / self.render_metrics.cell_size.width as usize;

            let size = PtySize {
                rows: rows as u16,
                cols: cols as u16,
                pixel_height: avail_height as u16,
                pixel_width: avail_width as u16,
            };

            (size, *dimensions)
        };

        self.render_state
            .advise_of_window_size_change(
                &self.render_metrics,
                dimensions.pixel_width,
                dimensions.pixel_height,
            )
            .expect("failed to advise of resize");

        self.terminal_size = size;

        let mux = Mux::get().unwrap();
        if let Some(window) = mux.get_window(self.mux_window_id) {
            for tab in window.iter() {
                tab.resize(size).ok();
            }
        };
        self.update_title();

        // Queue up a speculative resize in order to preserve the number of rows+cols
        if let Some(cell_dims) = scale_changed_cells {
            if let Some(window) = self.window.as_ref() {
                log::error!("scale changed so resize to {:?} {:?}", cell_dims, dims);
                window.set_inner_size(dims.pixel_width, dims.pixel_height);
            }
        }
    }

    fn current_cell_dimensions(&self) -> RowsAndCols {
        RowsAndCols {
            rows: self.terminal_size.rows as usize,
            cols: self.terminal_size.cols as usize,
        }
    }

    #[allow(clippy::float_cmp)]
    fn scaling_changed(&mut self, dimensions: Dimensions, font_scale: f64) {
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

    fn close_tab_idx(&mut self, idx: usize) -> anyhow::Result<()> {
        let mux = Mux::get().unwrap();
        if let Some(mut win) = mux.get_window_mut(self.mux_window_id) {
            let tab = win.remove_by_idx(idx);
            drop(win);
            mux.remove_tab(tab.tab_id());
        }
        self.activate_tab_relative(0)
    }

    fn paint_tab(&mut self, tab: &Rc<dyn Tab>, ctx: &mut dyn PaintContext) -> anyhow::Result<()> {
        let palette = tab.palette();
        let first_line_offset = if self.show_tab_bar { 1 } else { 0 };

        let mut term = tab.renderer();
        let cursor = term.get_cursor_position();
        self.prev_cursor.update(&cursor);
        let current_viewport = self.get_viewport(tab.tab_id());

        let dims = term.get_dimensions();

        if self.show_tab_bar {
            self.render_screen_line(
                ctx,
                0,
                None,
                self.tab_bar.line(),
                0..0,
                &cursor,
                &*term,
                &palette,
            )?;
        }

        {
            let stable_range = match current_viewport {
                Some(top) => top..top + dims.viewport_rows as StableRowIndex,
                None => dims.physical_top..dims.physical_top + dims.viewport_rows as StableRowIndex,
            };

            let (stable_top, lines) = term.get_lines(stable_range);

            for (line_idx, line) in lines.iter().enumerate() {
                let stable_row = stable_top + line_idx as StableRowIndex;

                let selrange = self
                    .selection(tab.tab_id())
                    .range
                    .map(|sel| sel.cols_for_row(stable_row))
                    .unwrap_or(0..0);

                self.render_screen_line(
                    ctx,
                    line_idx + first_line_offset,
                    Some(stable_row),
                    &line,
                    selrange,
                    &cursor,
                    &*term,
                    &palette,
                )?;
            }
        }

        // Fill any padding
        let config = configuration();
        let bg = rgbcolor_to_window_color(palette.background);
        // Fill any padding below the last row
        let dims = term.get_dimensions();
        let num_rows = dims.viewport_rows;
        let pixel_height_of_cells = config.window_padding.top as usize
            + (num_rows + first_line_offset) * self.render_metrics.cell_size.height as usize;
        ctx.clear_rect(
            Rect::new(
                Point::new(0, pixel_height_of_cells as isize),
                Size::new(
                    self.dimensions.pixel_width as isize,
                    (self
                        .dimensions
                        .pixel_height
                        .saturating_sub(pixel_height_of_cells)) as isize,
                ),
            ),
            bg,
        );

        // top padding
        ctx.clear_rect(
            Rect::new(
                Point::new(0, 0),
                Size::new(
                    self.dimensions.pixel_width as isize,
                    config.window_padding.top as isize,
                ),
            ),
            bg,
        );
        // left padding
        ctx.clear_rect(
            Rect::new(
                Point::new(0, config.window_padding.top as isize),
                Size::new(
                    config.window_padding.left as isize,
                    (self.dimensions.pixel_height - config.window_padding.top as usize) as isize,
                ),
            ),
            bg,
        );
        // right padding / scroll bar
        let padding_right = self.effective_right_padding(&config);

        ctx.clear_rect(
            Rect::new(
                Point::new(
                    (self.dimensions.pixel_width - padding_right as usize) as isize,
                    config.window_padding.top as isize,
                ),
                Size::new(
                    padding_right as isize,
                    (self.dimensions.pixel_height - config.window_padding.top as usize) as isize,
                ),
            ),
            bg,
        );

        if self.show_scroll_bar {
            let current_viewport = self.get_viewport(tab.tab_id());
            let info = ScrollHit::thumb(
                &*term,
                current_viewport,
                self.terminal_size,
                &self.dimensions,
            );

            let thumb_size = info.height as isize;
            let thumb_top = info.top as isize;

            ctx.clear_rect(
                Rect::new(
                    Point::new(
                        (self
                            .dimensions
                            .pixel_width
                            .saturating_sub(padding_right as usize))
                            as isize,
                        thumb_top,
                    ),
                    Size::new(padding_right as isize, thumb_size),
                ),
                rgbcolor_to_window_color(palette.scrollbar_thumb),
            );
        }

        Ok(())
    }

    fn effective_right_padding(&self, config: &ConfigHandle) -> u16 {
        effective_right_padding(config, &self.render_metrics)
    }

    fn paint_tab_opengl(
        &mut self,
        tab: &Rc<dyn Tab>,
        frame: &mut glium::Frame,
    ) -> anyhow::Result<()> {
        let palette = tab.palette();

        let background_color = palette.resolve_bg(term::color::ColorAttribute::Default);
        let (r, g, b, a) = background_color.to_tuple_rgba();
        frame.clear_color_srgb(r, g, b, a);

        let first_line_offset = if self.show_tab_bar { 1 } else { 0 };

        let mut term = tab.renderer();
        let cursor = term.get_cursor_position();
        self.prev_cursor.update(&cursor);

        let current_viewport = self.get_viewport(tab.tab_id());
        let (stable_top, lines);
        let dims = term.get_dimensions();

        {
            let stable_range = match current_viewport {
                Some(top) => top..top + dims.viewport_rows as StableRowIndex,
                None => dims.physical_top..dims.physical_top + dims.viewport_rows as StableRowIndex,
            };

            let (top, vp_lines) = term.get_lines(stable_range);
            stable_top = top;
            lines = vp_lines;
        }

        let gl_state = self.render_state.opengl();
        let mut vb = gl_state.glyph_vertex_buffer.borrow_mut();
        let mut quads = gl_state.quads.map(&mut vb);

        if self.show_tab_bar {
            self.render_screen_line_opengl(
                0,
                None,
                self.tab_bar.line(),
                0..0,
                &cursor,
                &*term,
                &palette,
                &mut quads,
            )?;
        }

        {
            let (thumb_top, thumb_size, color) = if self.show_scroll_bar {
                let info = ScrollHit::thumb(
                    &*term,
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

            let config = configuration();
            let padding = self.effective_right_padding(&config) as f32;

            let right = self.dimensions.pixel_width as f32 / 2.;
            let left = right - padding;

            let white_space = gl_state.util_sprites.white_space.texture_coords();

            quad.set_bg_color(color);
            quad.set_fg_color(color);
            quad.set_position(left, top, right, bottom);
            quad.set_texture(white_space);
            quad.set_texture_adjust(0., 0., 0., 0.);
            quad.set_underline(white_space);
            quad.set_has_color(false);
            quad.set_cursor(white_space);
            quad.set_cursor_color(rgbcolor_to_window_color(background_color));
        }

        for (line_idx, line) in lines.iter().enumerate() {
            let stable_row = stable_top + line_idx as StableRowIndex;
            let selrange = self
                .selection(tab.tab_id())
                .range
                .map(|sel| sel.cols_for_row(stable_row))
                .unwrap_or(0..0);

            self.render_screen_line_opengl(
                line_idx + first_line_offset,
                Some(stable_row),
                &line,
                selrange,
                &cursor,
                &*term,
                &palette,
                &mut quads,
            )?;
        }

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
            // No alpha blending for the background layer: let's make
            // sure that our background pixels are at 100% opacity.
            ..Default::default()
        };

        drop(quads);

        // Clamp and use the nearest texel rather than interpolate.
        // This prevents things like the box cursor outlines from
        // being randomly doubled in width or height
        let glyph_tex = Sampler::new(&*tex)
            .wrap_function(SamplerWrapFunction::Clamp)
            .magnify_filter(MagnifySamplerFilter::Nearest)
            .minify_filter(MinifySamplerFilter::Nearest);

        // Pass 1: Draw backgrounds, strikethrough and underline
        frame.draw(
            &*vb,
            &gl_state.glyph_index_buffer,
            &gl_state.program,
            &uniform! {
                projection: projection,
                glyph_tex:  glyph_tex,
                bg_and_line_layer: true,
            },
            &draw_params,
        )?;

        let draw_params = glium::DrawParameters {
            blend: glium::Blend {
                color: BlendingFunction::Addition {
                    source: LinearBlendingFactor::SourceAlpha,
                    destination: LinearBlendingFactor::OneMinusSourceAlpha,
                },
                alpha: BlendingFunction::Addition {
                    source: LinearBlendingFactor::SourceAlpha,
                    // On Wayland, the compositor takes the destination alpha
                    // value and blends with the window behind our own, which
                    // can make the text look brighter or less sharp.
                    // We set the destination alpha to 1.0 to prevent that
                    // from happening.
                    // (The normal alpha blending operation would set this to
                    // OneMinusSourceAlpha).
                    destination: LinearBlendingFactor::One,
                },
                constant_value: (0.0, 0.0, 0.0, 0.0),
            },
            ..Default::default()
        };

        // Pass 2: Draw glyphs
        frame.draw(
            &*vb,
            &gl_state.glyph_index_buffer,
            &gl_state.program,
            &uniform! {
                projection: projection,
                glyph_tex:  glyph_tex,
                bg_and_line_layer: false,
            },
            &draw_params,
        )?;

        Ok(())
    }

    fn lookup_cached_shape(&self, key: &ShapeCacheKey) -> Option<anyhow::Result<Vec<GlyphInfo>>> {
        match self.shape_cache.borrow_mut().get(key) {
            Some(Ok(info)) => Some(Ok(info.clone())),
            Some(Err(err)) => Some(Err(anyhow!("cached shaper error: {}", err))),
            None => None,
        }
    }

    /// "Render" a line of the terminal screen into the vertex buffer.
    /// This is nominally a matter of setting the fg/bg color and the
    /// texture coordinates for a given glyph.  There's a little bit
    /// of extra complexity to deal with multi-cell glyphs.
    #[allow(clippy::too_many_arguments)]
    fn render_screen_line_opengl(
        &self,
        line_idx: usize,
        stable_line_idx: Option<StableRowIndex>,
        line: &Line,
        selection: Range<usize>,
        cursor: &StableCursorPosition,
        terminal: &dyn Renderable,
        palette: &ColorPalette,
        quads: &mut MappedQuads,
    ) -> anyhow::Result<()> {
        let gl_state = self.render_state.opengl();

        let dims = terminal.get_dimensions();
        let num_cols = dims.cols;

        let cursor_border_color = rgbcolor_to_window_color(palette.cursor_border);

        // Break the line into clusters of cells with the same attributes
        let cell_clusters = line.cluster();
        let mut last_cell_idx = 0;
        let config = configuration();
        for cluster in cell_clusters {
            let attrs = &cluster.attrs;
            let is_highlited_hyperlink = match (&attrs.hyperlink, &self.current_highlight) {
                (&Some(ref this), &Some(ref highlight)) => Arc::ptr_eq(this, highlight),
                _ => false,
            };
            let style = self.fonts.match_style(&config, attrs);

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
                let key = ShapeCacheKey((style.clone(), cluster.text.clone()));
                match self.lookup_cached_shape(&key) {
                    Some(Ok(info)) => info,
                    Some(Err(err)) => return Err(err),
                    None => {
                        let font = self.fonts.resolve_font(style)?;
                        match font.shape(&cluster.text) {
                            Ok(info) => {
                                self.shape_cache.borrow_mut().put(key, Ok(info.clone()));
                                info
                            }
                            Err(err) => {
                                let res = anyhow!("shaper error: {}", err);
                                self.shape_cache.borrow_mut().put(key, Err(err));
                                return Err(res);
                            }
                        }
                    }
                }
            };

            for info in &glyph_info {
                let cell_idx = cluster.byte_to_cell_idx[info.cluster as usize];
                let glyph = gl_state
                    .glyph_cache
                    .borrow_mut()
                    .cached_glyph(info, style)?;

                let left = (glyph.x_offset + glyph.bearing_x).get() as f32;
                let top = ((PixelLength::new(self.render_metrics.cell_size.height as f64)
                    + self.render_metrics.descender)
                    - (glyph.y_offset + glyph.bearing_y))
                    .get() as f32;

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

                    let (glyph_color, bg_color, cursor_shape) = self.compute_cell_fg_bg(
                        stable_line_idx,
                        cell_idx,
                        cursor,
                        &selection,
                        glyph_color,
                        bg_color,
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

                            let mut quad = match quads.cell(cell_idx, line_idx) {
                                Ok(quad) => quad,
                                Err(_) => break,
                            };

                            quad.set_fg_color(glyph_color);
                            quad.set_bg_color(bg_color);
                            quad.set_texture(texture_rect);
                            quad.set_texture_adjust(0., 0., 0., 0.);
                            quad.set_underline(gl_state.util_sprites.white_space.texture_coords());
                            quad.set_has_color(true);
                            quad.set_cursor(
                                gl_state
                                    .util_sprites
                                    .cursor_sprite(cursor_shape)
                                    .texture_coords(),
                            );
                            quad.set_cursor_color(cursor_border_color);

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

                    let mut quad = match quads.cell(cell_idx, line_idx) {
                        Ok(quad) => quad,
                        Err(_) => break,
                    };

                    quad.set_fg_color(glyph_color);
                    quad.set_bg_color(bg_color);
                    quad.set_texture(texture_rect);
                    quad.set_texture_adjust(left, top, right, bottom);
                    quad.set_underline(underline_tex_rect);
                    quad.set_has_color(glyph.has_color);
                    quad.set_cursor(
                        gl_state
                            .util_sprites
                            .cursor_sprite(cursor_shape)
                            .texture_coords(),
                    );
                    quad.set_cursor_color(cursor_border_color);
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
            let (glyph_color, bg_color, cursor_shape) = self.compute_cell_fg_bg(
                stable_line_idx,
                cell_idx,
                cursor,
                &selection,
                rgbcolor_to_window_color(palette.foreground),
                rgbcolor_to_window_color(palette.background),
                palette,
            );

            let mut quad = match quads.cell(cell_idx, line_idx) {
                Ok(quad) => quad,
                Err(_) => break,
            };

            quad.set_bg_color(bg_color);
            quad.set_fg_color(glyph_color);
            quad.set_texture(white_space);
            quad.set_texture_adjust(0., 0., 0., 0.);
            quad.set_underline(white_space);
            quad.set_has_color(false);
            quad.set_cursor(
                gl_state
                    .util_sprites
                    .cursor_sprite(cursor_shape)
                    .texture_coords(),
            );
            quad.set_cursor_color(cursor_border_color);
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn render_screen_line(
        &self,
        ctx: &mut dyn PaintContext,
        line_idx: usize,
        stable_line_idx: Option<StableRowIndex>,
        line: &Line,
        selection: Range<usize>,
        cursor: &StableCursorPosition,
        terminal: &dyn Renderable,
        palette: &ColorPalette,
    ) -> anyhow::Result<()> {
        let config = configuration();

        let padding_left = config.window_padding.left as isize;
        let padding_top = config.window_padding.top as isize;

        let dims = terminal.get_dimensions();
        let num_cols = dims.cols;
        let cursor_border_color = rgbcolor_to_window_color(palette.cursor_border);

        // Break the line into clusters of cells with the same attributes
        let cell_clusters = line.cluster();
        let mut last_cell_idx = 0;
        for cluster in cell_clusters {
            let attrs = &cluster.attrs;
            let is_highlited_hyperlink = match (&attrs.hyperlink, &self.current_highlight) {
                (&Some(ref this), &Some(ref highlight)) => this == highlight,
                _ => false,
            };
            let style = self.fonts.match_style(&config, attrs);

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
                let font = self.fonts.resolve_font(style)?;
                font.shape(&cluster.text)?
            };

            for info in &glyph_info {
                let cell_idx = cluster.byte_to_cell_idx[info.cluster as usize];
                let glyph = self.render_state.cached_software_glyph(info, style)?;

                let left = (glyph.x_offset + glyph.bearing_x).get() as f32;
                let top = ((PixelLength::new(self.render_metrics.cell_size.to_f64().height)
                    + self.render_metrics.descender)
                    - (glyph.y_offset + glyph.bearing_y))
                    .get() as f32;

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

                    let (glyph_color, bg_color, cursor_shape) = self.compute_cell_fg_bg(
                        stable_line_idx,
                        cell_idx,
                        cursor,
                        &selection,
                        glyph_color,
                        bg_color,
                        palette,
                    );

                    let cell_rect = Rect::new(
                        Point::new(
                            (cell_idx as isize * self.render_metrics.cell_size.width)
                                + padding_left,
                            (self.render_metrics.cell_size.height * line_idx as isize)
                                + padding_top,
                        ),
                        self.render_metrics.cell_size,
                    );
                    ctx.clear_rect(cell_rect, bg_color);

                    {
                        let software = self.render_state.software();
                        let sprite = software.util_sprites.select_sprite(
                            is_highlited_hyperlink,
                            attrs.strikethrough(),
                            underline,
                        );
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

                    if cursor_shape != CursorShape::Hidden {
                        let software = self.render_state.software();
                        let sprite = software.util_sprites.cursor_sprite(cursor_shape);
                        ctx.draw_image(
                            cell_rect.origin,
                            Some(sprite.coords),
                            &*sprite.texture.image.borrow(),
                            Operator::MultiplyThenOver(cursor_border_color),
                        );
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
            let (_glyph_color, bg_color, cursor_shape) = self.compute_cell_fg_bg(
                stable_line_idx,
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

            if cursor_shape != CursorShape::Hidden {
                let software = self.render_state.software();
                let sprite = software.util_sprites.cursor_sprite(cursor_shape);
                ctx.draw_image(
                    cell_rect.origin,
                    Some(sprite.coords),
                    &*sprite.texture.image.borrow(),
                    Operator::MultiplyThenOver(cursor_border_color),
                );
            }
        }

        // Fill any marginal area to the right of the last cell
        let pixel_width_of_cells =
            padding_left as usize + (num_cols * self.render_metrics.cell_size.width as usize);
        ctx.clear_rect(
            Rect::new(
                Point::new(
                    pixel_width_of_cells as isize,
                    self.render_metrics.cell_size.height * line_idx as isize,
                ),
                Size::new(
                    self.dimensions
                        .pixel_width
                        .saturating_sub(pixel_width_of_cells) as isize,
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
        stable_line_idx: Option<StableRowIndex>,
        cell_idx: usize,
        cursor: &StableCursorPosition,
        selection: &Range<usize>,
        fg_color: Color,
        bg_color: Color,
        palette: &ColorPalette,
    ) -> (Color, Color, CursorShape) {
        let selected = selection.contains(&cell_idx);

        let is_cursor = stable_line_idx == Some(cursor.y) && cursor.x == cell_idx;

        let cursor_shape = if is_cursor {
            // This logic figures out whether the cursor is visible or not.
            // If the cursor is explicitly hidden then it is obviously not
            // visible.
            // If the cursor is set to a blinking mode then we are visible
            // depending on the current time.
            let config = configuration();
            let shape = config.default_cursor_style.effective_shape(cursor.shape);
            // Work out the blinking shape if its a blinking cursor and it hasn't been disabled
            // and the window is focused.
            let blinking =
                shape.is_blinking() && config.cursor_blink_rate != 0 && self.focused.is_some();
            if blinking {
                // Divide the time since we last moved by the blink rate.
                // If the result is even then the cursor is "on", else it
                // is "off"
                let now = std::time::Instant::now();
                let milli_uptime = now
                    .duration_since(self.prev_cursor.last_cursor_movement())
                    .as_millis();
                let ticks = milli_uptime / config.cursor_blink_rate as u128;
                if (ticks & 1) == 0 {
                    shape
                } else {
                    CursorShape::Hidden
                }
            } else {
                shape
            }
        } else {
            CursorShape::Hidden
        };

        let (fg_color, bg_color) = match (selected, self.focused.is_some(), cursor_shape) {
            // Selected text overrides colors
            (true, _, CursorShape::Hidden) => (
                rgbcolor_to_window_color(palette.selection_fg),
                rgbcolor_to_window_color(palette.selection_bg),
            ),
            // Cursor cell overrides colors
            (_, true, CursorShape::BlinkingBlock) | (_, true, CursorShape::SteadyBlock) => (
                rgbcolor_to_window_color(palette.cursor_fg),
                rgbcolor_to_window_color(palette.cursor_bg),
            ),
            // Normally, render the cell as configured (or if the window is unfocused)
            _ => (fg_color, bg_color),
        };

        (fg_color, bg_color, cursor_shape)
    }

    fn tab_state(&self, tab_id: TabId) -> RefMut<TabState> {
        RefMut::map(self.tab_state.borrow_mut(), |state| {
            state.entry(tab_id).or_insert_with(TabState::default)
        })
    }

    fn selection(&self, tab_id: TabId) -> RefMut<Selection> {
        RefMut::map(self.tab_state(tab_id), |state| &mut state.selection)
    }

    fn get_viewport(&self, tab_id: TabId) -> Option<StableRowIndex> {
        self.tab_state(tab_id).viewport
    }

    fn set_viewport(
        &mut self,
        tab_id: TabId,
        position: Option<StableRowIndex>,
        dims: RenderableDimensions,
    ) {
        let pos = match position {
            Some(pos) => {
                // Drop out of scrolling mode if we're off the bottom
                if pos >= dims.physical_top {
                    None
                } else {
                    Some(pos.max(0))
                }
            }
            None => None,
        };
        self.tab_state(tab_id).viewport = pos;
    }

    fn mouse_event_tab_bar(&mut self, x: usize, event: &MouseEvent, context: &dyn WindowOps) {
        match event.kind {
            WMEK::Press(MousePress::Left) => match self.tab_bar.hit_test(x) {
                TabBarItem::Tab(tab_idx) => {
                    self.activate_tab(tab_idx).ok();
                }
                TabBarItem::NewTabButton => {
                    self.spawn_tab(&SpawnTabDomain::CurrentTabDomain);
                }
                TabBarItem::None => {}
            },
            WMEK::Press(MousePress::Middle) => match self.tab_bar.hit_test(x) {
                TabBarItem::Tab(tab_idx) => {
                    self.close_tab_idx(tab_idx).ok();
                }
                TabBarItem::NewTabButton | TabBarItem::None => {}
            },
            WMEK::Press(MousePress::Right) => match self.tab_bar.hit_test(x) {
                TabBarItem::Tab(_) => {
                    self.show_tab_navigator();
                }
                TabBarItem::NewTabButton => {
                    self.show_launcher();
                }
                TabBarItem::None => {}
            },
            _ => {}
        }
        self.update_title();
        context.set_cursor(Some(MouseCursor::Arrow));
    }

    fn mouse_event_scroll_bar(
        &mut self,
        tab: Rc<dyn Tab>,
        event: &MouseEvent,
        context: &dyn WindowOps,
    ) {
        if let WMEK::Press(MousePress::Left) = event.kind {
            let render = tab.renderer();
            let dims = render.get_dimensions();
            let current_viewport = self.get_viewport(tab.tab_id());

            match ScrollHit::test(
                event.coords.y,
                &*render,
                current_viewport,
                self.terminal_size,
                &self.dimensions,
            ) {
                ScrollHit::Above => {
                    // Page up
                    self.set_viewport(
                        tab.tab_id(),
                        Some(
                            current_viewport
                                .unwrap_or(dims.physical_top)
                                .saturating_sub(self.terminal_size.rows.try_into().unwrap()),
                        ),
                        dims,
                    );
                    context.invalidate();
                }
                ScrollHit::Below => {
                    // Page down
                    self.set_viewport(
                        tab.tab_id(),
                        Some(
                            current_viewport
                                .unwrap_or(dims.physical_top)
                                .saturating_add(self.terminal_size.rows.try_into().unwrap()),
                        ),
                        dims,
                    );
                    context.invalidate();
                }
                ScrollHit::OnThumb(from_top) => {
                    // Start a scroll drag
                    self.scroll_drag_start = Some(from_top);
                }
            };
        }
        context.set_cursor(Some(MouseCursor::Arrow));
    }

    fn triple_click_left(&mut self, tab: &Rc<dyn Tab>, x: usize, y: StableRowIndex) {
        let start = SelectionCoordinate { x, y };
        let selection_range = SelectionRange::line_around(start);

        self.selection(tab.tab_id()).start = Some(start);
        self.selection(tab.tab_id()).range = Some(selection_range);
    }

    fn double_click_left(&mut self, tab: &Rc<dyn Tab>, x: usize, row: StableRowIndex) {
        let selection_range =
            SelectionRange::word_around(SelectionCoordinate { x, y: row }, &mut *tab.renderer());

        self.selection(tab.tab_id()).start = Some(selection_range.start);
        self.selection(tab.tab_id()).range = Some(selection_range);
    }

    fn mouse_event_terminal(
        &mut self,
        tab: Rc<dyn Tab>,
        x: usize,
        y: i64,
        event: &MouseEvent,
        context: &dyn WindowOps,
    ) {
        let dims = tab.renderer().get_dimensions();
        let stable_row =
            self.get_viewport(tab.tab_id()).unwrap_or(dims.physical_top) + y as StableRowIndex;

        let (top, mut lines) = tab.renderer().get_lines(stable_row..stable_row + 1);
        let new_highlight = if top == stable_row {
            if let Some(line) = lines.get_mut(0) {
                if let Some(cell) = line.cells().get(x) {
                    cell.attrs().hyperlink.as_ref().cloned()
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        match (self.current_highlight.as_ref(), new_highlight) {
            (Some(old_link), Some(new_link)) if Arc::ptr_eq(&old_link, &new_link) => {
                // Unchanged
            }
            (_, rhs) => {
                // We're hovering over a different URL, so invalidate and repaint
                // so that we render the underline correctly
                self.current_highlight = rhs;
                context.invalidate();
            }
        };

        context.set_cursor(Some(if self.current_highlight.is_some() {
            // When hovering over a hyperlink, show an appropriate
            // mouse cursor to give the cue that it is clickable
            MouseCursor::Hand
        } else {
            MouseCursor::Text
        }));

        if !tab.is_mouse_grabbed() || event.modifiers == Modifiers::SHIFT {
            match (&event.kind, self.last_mouse_click.as_ref()) {
                // Triple click to select a word
                (
                    WMEK::Press(MousePress::Left),
                    Some(LastMouseClick {
                        streak: 3,
                        button: TMB::Left,
                        ..
                    }),
                ) => {
                    self.triple_click_left(&tab, x, stable_row);
                    context.invalidate();
                }
                // Double click to select a word
                (
                    WMEK::Press(MousePress::Left),
                    Some(LastMouseClick {
                        streak: 2,
                        button: TMB::Left,
                        ..
                    }),
                ) => {
                    self.double_click_left(&tab, x, stable_row);
                    context.invalidate();
                }

                // Left single click to initiate a selection drag
                // Shift+Left click to extend a selection
                (
                    WMEK::Press(MousePress::Left),
                    Some(LastMouseClick {
                        streak: 1,
                        button: TMB::Left,
                        ..
                    }),
                ) => {
                    // If the mouse is grabbed, do not use Shfit+Left to
                    // extend a selection, since otherwise it's hard to
                    // clear a selection.
                    if tab.is_mouse_grabbed()
                        || !event.modifiers.contains(Modifiers::SHIFT)
                        || self.selection(tab.tab_id()).is_empty()
                    {
                        // Initiate a selection
                        self.selection(tab.tab_id())
                            .begin(SelectionCoordinate { x, y: stable_row });
                    } else {
                        // Extend selection
                        let end = SelectionCoordinate { x, y: stable_row };
                        let mut selection = self.selection(tab.tab_id());
                        selection.range = selection.range.map(|sel| sel.extend(end));
                        context.invalidate();
                    }
                }

                // Release button to finish a selection
                (
                    WMEK::Release(MousePress::Left),
                    Some(LastMouseClick {
                        streak: 1,
                        button: TMB::Left,
                        ..
                    }),
                ) => {
                    let text = self.selection_text(&tab);

                    if text.is_empty() && self.current_highlight.is_some() {
                        // They clicked on a link, so let's open it!
                        // Ensure that we spawn the `open` call outside of the context
                        // of our window loop; on Windows it can cause a panic due to
                        // triggering our WndProc recursively.
                        let link = self.current_highlight.as_ref().unwrap().clone();
                        promise::spawn::spawn(async move {
                            log::error!("clicking {}", link.uri());
                            if let Err(err) = open::that(link.uri()) {
                                log::error!("failed to open {}: {:?}", link.uri(), err);
                            }
                        });
                    } else if !text.is_empty() {
                        self.window.as_ref().unwrap().set_clipboard(text);
                        context.invalidate();
                    }
                }

                // Release button to finish a selection (word and line mode)
                (
                    WMEK::Release(MousePress::Left),
                    Some(LastMouseClick {
                        streak,
                        button: TMB::Left,
                        ..
                    }),
                ) if *streak > 1 => {
                    let text = self.selection_text(&tab);

                    self.window.as_ref().unwrap().set_clipboard(text);
                    context.invalidate();
                }

                // Dragging left mouse button
                (
                    WMEK::Move,
                    Some(LastMouseClick {
                        streak: 1,
                        button: TMB::Left,
                        ..
                    }),
                ) => {
                    if let Some(MousePress::Left) = self.current_mouse_button {
                        let end = SelectionCoordinate { x, y: stable_row };
                        let selection_range = self.selection(tab.tab_id()).range.take();
                        let sel = match selection_range {
                            None => SelectionRange::start(
                                self.selection(tab.tab_id()).start.unwrap_or(end),
                            )
                            .extend(end),
                            Some(sel) => sel.extend(end),
                        };
                        self.selection(tab.tab_id()).range = Some(sel);
                        context.invalidate();
                    }
                }

                // Dragging left mouse button in word mode
                (
                    WMEK::Move,
                    Some(LastMouseClick {
                        streak: 2,
                        button: TMB::Left,
                        ..
                    }),
                ) => {
                    if let Some(MousePress::Left) = self.current_mouse_button {
                        let end_word = SelectionRange::word_around(
                            SelectionCoordinate { x, y: stable_row },
                            &mut *tab.renderer(),
                        );

                        let start_coord = self
                            .selection(tab.tab_id())
                            .start
                            .clone()
                            .unwrap_or(end_word.start);
                        let start_word =
                            SelectionRange::word_around(start_coord, &mut *tab.renderer());

                        let selection_range = start_word.extend_with(end_word);
                        self.selection(tab.tab_id()).range = Some(selection_range);
                        context.invalidate();
                    }
                }

                // Dragging left mouse button in line mode
                (
                    WMEK::Move,
                    Some(LastMouseClick {
                        streak: 3,
                        button: TMB::Left,
                        ..
                    }),
                ) => {
                    if let Some(MousePress::Left) = self.current_mouse_button {
                        let end_line =
                            SelectionRange::line_around(SelectionCoordinate { x, y: stable_row });

                        let start_coord = self
                            .selection(tab.tab_id())
                            .start
                            .clone()
                            .unwrap_or(end_line.start);
                        let start_line = SelectionRange::line_around(start_coord);

                        let selection_range = start_line.extend_with(end_line);
                        self.selection(tab.tab_id()).range = Some(selection_range);
                        context.invalidate();
                    }
                }

                // Middle mouse button is Paste
                (
                    WMEK::Press(MousePress::Middle),
                    Some(LastMouseClick {
                        streak: 1,
                        button: TMB::Middle,
                        ..
                    }),
                ) => {
                    let tab_id = tab.tab_id();
                    let future = self.window.as_ref().unwrap().get_clipboard();
                    promise::spawn::spawn(async move {
                        if let Ok(clip) = future.await {
                            let mux = Mux::get().unwrap();
                            if let Some(tab) = mux.get_tab(tab_id) {
                                tab.trickle_paste(clip).ok();
                            }
                        }
                    });
                    return;
                }
                _ => {}
            }

            return;
        }

        let mouse_event = term::MouseEvent {
            kind: match event.kind {
                WMEK::Move => TMEK::Move,
                WMEK::VertWheel(_) | WMEK::HorzWheel(_) | WMEK::Press(_) => TMEK::Press,
                WMEK::Release(_) => TMEK::Release,
            },
            button: match event.kind {
                WMEK::Release(ref press) | WMEK::Press(ref press) => match press {
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
            x,
            y,
            modifiers: window_mods_to_termwiz_mods(event.modifiers),
        };

        tab.mouse_event(
            mouse_event,
            &mut Host {
                writer: &mut *tab.writer(),
            },
        )
        .ok();

        match event.kind {
            WMEK::Move => {}
            _ => {
                context.invalidate();
            }
        }
    }

    fn maybe_scroll_to_bottom_for_input(&mut self, tab: &Rc<dyn Tab>) {
        if configuration().scroll_to_bottom_on_input {
            self.scroll_to_bottom(tab);
        }
    }

    fn scroll_to_bottom(&mut self, tab: &Rc<dyn Tab>) {
        self.tab_state(tab.tab_id()).viewport = None;
    }

    /// Returns a Tab that we can interact with; this will typically be
    /// the active tab for the window, but if the window has an overlay,
    /// then that will be returned instead.
    fn get_active_tab_or_overlay(&self) -> Option<Rc<dyn Tab>> {
        let mux = Mux::get().unwrap();
        let tab = match mux.get_active_tab_for_window(self.mux_window_id) {
            Some(tab) => tab,
            None => return None,
        };

        let tab_id = tab.tab_id();
        self.tab_state(tab_id).overlay.clone().or_else(|| Some(tab))
    }

    /// Removes any overlay for the specified tab
    fn cancel_overlay_for_tab(&self, tab_id: TabId) {
        self.tab_state(tab_id).overlay.take();
        if let Some(window) = self.window.as_ref() {
            window.invalidate();
        }
    }

    pub fn schedule_cancel_overlay(window: Window, tab_id: TabId) {
        window.apply(move |myself, _| {
            if let Some(myself) = myself.downcast_mut::<Self>() {
                myself.cancel_overlay_for_tab(tab_id);
            }
            Ok(())
        });
    }

    pub fn assign_overlay(&self, tab_id: TabId, overlay: Rc<dyn Tab>) {
        self.tab_state(tab_id).overlay.replace(overlay);
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
