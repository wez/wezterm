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
use crate::frontend::gui::overlay::{
    launcher, start_overlay, tab_navigator, CopyOverlay, SearchOverlay,
};
use crate::frontend::gui::scrollbar::*;
use crate::frontend::gui::selection::*;
use crate::frontend::gui::tabbar::{TabBarItem, TabBarState};
use crate::keyassignment::{
    InputMap, KeyAssignment, MouseEventTrigger, SpawnCommand, SpawnTabDomain,
};
use crate::mux::domain::{DomainId, DomainState};
use crate::mux::renderable::{RenderableDimensions, StableCursorPosition};
use crate::mux::tab::{Pane, PaneId, PositionedPane, PositionedSplit, SplitDirection, Tab, TabId};
use crate::mux::window::WindowId as MuxWindowId;
use crate::mux::Mux;
use ::wezterm_term::input::MouseButton as TMB;
use ::wezterm_term::input::MouseEventKind as TMEK;
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
use std::ops::{Add, Range, Sub};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use termwiz::color::RgbColor;
use termwiz::hyperlink::Hyperlink;
use termwiz::surface::{CursorShape, CursorVisibility};
use wezterm_term::color::ColorPalette;
use wezterm_term::input::LastMouseClick;
use wezterm_term::{CellAttributes, Line, StableRowIndex, Underline};

const ATLAS_SIZE: usize = 4096;

#[derive(Copy, Debug, Clone, Eq, PartialEq)]
pub enum SpawnWhere {
    NewWindow,
    NewTab,
    SplitPane(SplitDirection),
}

struct RenderScreenLineOpenGLParams<'a> {
    line_idx: usize,
    stable_line_idx: Option<StableRowIndex>,
    line: &'a Line,
    selection: Range<usize>,
    cursor: &'a StableCursorPosition,
    palette: &'a ColorPalette,
    dims: &'a RenderableDimensions,
    config: &'a ConfigHandle,
    pos: &'a PositionedPane,

    cursor_border_color: Color,
    foreground: Color,
    background: Color,
}

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

impl wezterm_term::Clipboard for ClipboardHelper {
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

#[derive(Clone)]
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

#[derive(Default, Clone)]
pub struct PaneState {
    /// If is_some(), the top row of the visible screen.
    /// Otherwise, the viewport is at the bottom of the
    /// scrollback.
    viewport: Option<StableRowIndex>,
    selection: Selection,
    /// If is_some(), rather than display the actual tab
    /// contents, we're overlaying a little internal application
    /// tab.  We'll also route input to it.
    pub overlay: Option<Rc<dyn Pane>>,
}

#[derive(Default, Clone)]
pub struct TabState {
    /// If is_some(), rather than display the actual tab
    /// contents, we're overlaying a little internal application
    /// tab.  We'll also route input to it.
    pub overlay: Option<Rc<dyn Pane>>,
}

#[derive(PartialEq, Eq, Hash)]
struct ShapeCacheKey {
    style: TextStyle,
    text: String,
}

/// We'd like to avoid allocating when resolving from the cache
/// so this is the borrowed version of ShapeCacheKey.
/// It's a bit involved to make this work; more details can be
/// found in the excellent guide here:
/// <https://github.com/sunshowers/borrow-complex-key-example/blob/master/src/lib.rs>
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
struct BorrowedShapeCacheKey<'a> {
    style: &'a TextStyle,
    text: &'a str,
}

impl<'a> BorrowedShapeCacheKey<'a> {
    fn to_owned(&self) -> ShapeCacheKey {
        ShapeCacheKey {
            style: self.style.clone(),
            text: self.text.to_owned(),
        }
    }
}

trait ShapeCacheKeyTrait {
    fn key<'k>(&'k self) -> BorrowedShapeCacheKey<'k>;
}

impl ShapeCacheKeyTrait for ShapeCacheKey {
    fn key<'k>(&'k self) -> BorrowedShapeCacheKey<'k> {
        BorrowedShapeCacheKey {
            style: &self.style,
            text: &self.text,
        }
    }
}

impl<'a> ShapeCacheKeyTrait for BorrowedShapeCacheKey<'a> {
    fn key<'k>(&'k self) -> BorrowedShapeCacheKey<'k> {
        *self
    }
}

impl<'a> std::borrow::Borrow<dyn ShapeCacheKeyTrait + 'a> for ShapeCacheKey {
    fn borrow(&self) -> &(dyn ShapeCacheKeyTrait + 'a) {
        self
    }
}

impl<'a> std::borrow::Borrow<dyn ShapeCacheKeyTrait + 'a> for lru::KeyRef<ShapeCacheKey> {
    fn borrow(&self) -> &(dyn ShapeCacheKeyTrait + 'a) {
        let k: &ShapeCacheKey = self.borrow();
        k
    }
}

impl<'a> PartialEq for (dyn ShapeCacheKeyTrait + 'a) {
    fn eq(&self, other: &Self) -> bool {
        self.key().eq(&other.key())
    }
}

impl<'a> Eq for (dyn ShapeCacheKeyTrait + 'a) {}

impl<'a> std::hash::Hash for (dyn ShapeCacheKeyTrait + 'a) {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.key().hash(state)
    }
}

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
    input_map: InputMap,
    /// If is_some, the LEADER modifier is active until the specified instant.
    leader_is_down: Option<std::time::Instant>,
    show_tab_bar: bool,
    show_scroll_bar: bool,
    tab_bar: TabBarState,
    last_mouse_coords: (usize, i64),
    last_mouse_terminal_coords: (usize, StableRowIndex),
    scroll_drag_start: Option<isize>,
    split_drag_start: Option<PositionedSplit>,
    config_generation: usize,
    prev_cursor: PrevCursorPos,
    last_scroll_info: RenderableDimensions,

    tab_state: RefCell<HashMap<TabId, TabState>>,
    pane_state: RefCell<HashMap<PaneId, PaneState>>,

    /// Gross workaround for managing async keyboard fetching
    /// just for middle mouse button paste function
    clipboard_contents: Arc<Mutex<Option<String>>>,

    current_mouse_button: Option<MousePress>,

    /// Keeps track of double and triple clicks
    last_mouse_click: Option<LastMouseClick>,

    /// The URL over which we are currently hovering
    current_highlight: Option<Arc<Hyperlink>>,

    shape_cache: RefCell<LruCache<ShapeCacheKey, anyhow::Result<Rc<Vec<GlyphInfo>>>>>,

    last_blink_paint: Instant,
}

fn mouse_press_to_tmb(press: &MousePress) -> TMB {
    match press {
        MousePress::Left => TMB::Left,
        MousePress::Right => TMB::Right,
        MousePress::Middle => TMB::Middle,
    }
}

#[derive(Debug)]
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

        if let Some(pane) = self.get_active_pane_or_overlay() {
            pane.focus_changed(focused);
        }
    }

    fn mouse_event(&mut self, event: &MouseEvent, context: &dyn WindowOps) {
        let pane = match self.get_active_pane_or_overlay() {
            Some(pane) => pane,
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
                    // Completed a scrollbar drag
                    return;
                }
                if press == &MousePress::Left && self.split_drag_start.take().is_some() {
                    // Completed a split drag
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
                let button = mouse_press_to_tmb(press);

                let click = match self.last_mouse_click.take() {
                    None => LastMouseClick::new(button),
                    Some(click) => click.add(button),
                };
                self.last_mouse_click = Some(click);
                self.current_mouse_button = Some(press.clone());
            }

            WMEK::VertWheel(amount) if !pane.is_mouse_grabbed() => {
                // adjust viewport
                let dims = pane.renderer().get_dimensions();
                let position = self
                    .get_viewport(pane.pane_id())
                    .unwrap_or(dims.physical_top)
                    .saturating_sub(amount.into());
                self.set_viewport(pane.pane_id(), Some(position), dims);
                context.invalidate();
                return;
            }

            WMEK::Move => {
                let current_viewport = self.get_viewport(pane.pane_id());
                if let Some(from_top) = self.scroll_drag_start.as_ref() {
                    // Dragging the scroll bar
                    let pane = match self.get_active_pane_or_overlay() {
                        Some(pane) => pane,
                        None => return,
                    };

                    let render = pane.renderer();
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
                    drop(render);
                    self.set_viewport(pane.pane_id(), Some(row), dims);
                    context.invalidate();
                    return;
                }

                if let Some(split) = self.split_drag_start.take() {
                    let mux = Mux::get().unwrap();
                    let tab = match mux.get_active_tab_for_window(self.mux_window_id) {
                        Some(tab) => tab,
                        None => return,
                    };
                    let delta = match split.direction {
                        SplitDirection::Horizontal => {
                            (x as isize).saturating_sub(split.left as isize)
                        }
                        SplitDirection::Vertical => {
                            (term_y as isize).saturating_sub(split.top as isize)
                        }
                    };

                    if delta != 0 {
                        tab.resize_split_by(split.index, delta);

                        self.split_drag_start = tab.iter_splits().into_iter().nth(split.index);
                        context.invalidate();
                    } else {
                        self.split_drag_start.replace(split);
                    }

                    return;
                }
            }
            _ => {}
        }

        if in_tab_bar {
            self.mouse_event_tab_bar(x, event, context);
        } else if in_scroll_bar {
            self.mouse_event_scroll_bar(pane, event, context);
        } else {
            self.mouse_event_terminal(pane, x, term_y, event, context);
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

    fn key_event(&mut self, window_key: &KeyEvent, context: &dyn WindowOps) -> bool {
        if !window_key.key_is_down {
            return false;
        }

        // log::error!("key_event {:?}", key);

        let pane = match self.get_active_pane_or_overlay() {
            Some(pane) => pane,
            None => return false,
        };

        // The leader key is a kind of modal modifier key.
        // It is allowed to be active for up to the leader timeout duration,
        // after which it auto-deactivates.
        let (leader_active, leader_mod) = match self.leader_is_down.as_ref() {
            Some(expiry) if *expiry > std::time::Instant::now() => {
                // Currently active
                (true, termwiz::input::Modifiers::LEADER)
            }
            Some(_) => {
                // Expired; clear out the old expiration time
                self.leader_is_down.take();
                (false, termwiz::input::Modifiers::NONE)
            }
            _ => (false, termwiz::input::Modifiers::NONE),
        };

        let modifiers = window_mods_to_termwiz_mods(window_key.modifiers);
        let raw_modifiers = window_mods_to_termwiz_mods(window_key.raw_modifiers);

        // First chance to operate on the raw key; if it matches a
        // user-defined key binding then we execute it and stop there.
        if let Some(key) = &window_key.raw_key {
            if let Key::Code(key) = self.win_key_code_to_termwiz_key_code(&key) {
                if !leader_active {
                    // Check to see if this key-press is the leader activating
                    if let Some(duration) = self.input_map.is_leader(key, raw_modifiers) {
                        // Yes; record its expiration
                        self.leader_is_down
                            .replace(std::time::Instant::now() + duration);
                        return true;
                    }
                }

                if let Some(assignment) = self.input_map.lookup_key(key, raw_modifiers | leader_mod)
                {
                    self.perform_key_assignment(&pane, &assignment).ok();
                    context.invalidate();

                    if leader_active {
                        // A successful leader key-lookup cancels the leader
                        // virtual modifier state
                        self.leader_is_down.take();
                    }
                    return true;
                }

                // While the leader modifier is active, only registered
                // keybindings are recognized.
                if !leader_active {
                    let config = configuration();

                    // This is a bit ugly.
                    // Not all of our platforms report LEFT|RIGHT ALT; most report just ALT.
                    // For those that do distinguish between them we want to respect the left vs.
                    // right settings for the compose behavior.
                    // Otherwise, if the event didn't include left vs. right then we want to
                    // respect the generic compose behavior.
                    let bypass_compose =
                    // Left ALT and they disabled compose
                    (window_key.raw_modifiers.contains(Modifiers::LEFT_ALT)
                    && !config.send_composed_key_when_left_alt_is_pressed)
                    // Right ALT and they disabled compose
                    || (window_key.raw_modifiers.contains(Modifiers::RIGHT_ALT)
                        && !config.send_composed_key_when_right_alt_is_pressed)
                    // Generic ALT and they disabled generic compose
                    || (!window_key.raw_modifiers.contains(Modifiers::RIGHT_ALT)
                        && !window_key.raw_modifiers.contains(Modifiers::LEFT_ALT)
                        && window_key.raw_modifiers.contains(Modifiers::ALT)
                        && !config.send_composed_key_when_alt_is_pressed);

                    if bypass_compose && pane.key_down(key, raw_modifiers).is_ok() {
                        if !key.is_modifier() && self.pane_state(pane.pane_id()).overlay.is_none() {
                            self.maybe_scroll_to_bottom_for_input(&pane);
                        }
                        context.invalidate();
                        return true;
                    }
                }
            }
        }

        let key = self.win_key_code_to_termwiz_key_code(&window_key.key);
        match key {
            Key::Code(key) => {
                if !leader_active {
                    // Check to see if this key-press is the leader activating
                    if let Some(duration) = self.input_map.is_leader(key, modifiers) {
                        // Yes; record its expiration
                        self.leader_is_down
                            .replace(std::time::Instant::now() + duration);
                        return true;
                    }
                }

                if let Some(assignment) = self.input_map.lookup_key(key, modifiers | leader_mod) {
                    self.perform_key_assignment(&pane, &assignment).ok();
                    context.invalidate();
                    if leader_active {
                        // A successful leader key-lookup cancels the leader
                        // virtual modifier state
                        self.leader_is_down.take();
                    }
                    true
                } else if leader_active {
                    if !key.is_modifier() {
                        // Leader was pressed and this non-modifier keypress isn't
                        // a registered key binding; swallow this event and cancel
                        // the leader modifier
                        self.leader_is_down.take();
                    }
                    true
                } else if pane.key_down(key, modifiers).is_ok() {
                    if !key.is_modifier() && self.pane_state(pane.pane_id()).overlay.is_none() {
                        self.maybe_scroll_to_bottom_for_input(&pane);
                    }
                    context.invalidate();
                    true
                } else {
                    false
                }
            }
            Key::Composed(s) => {
                if leader_active {
                    // Leader was pressed and this non-modifier keypress isn't
                    // a registered key binding; swallow this event and cancel
                    // the leader modifier.
                    self.leader_is_down.take();
                } else {
                    pane.writer().write_all(s.as_bytes()).ok();
                    self.maybe_scroll_to_bottom_for_input(&pane);
                    context.invalidate();
                }
                true
            }
            Key::None => false,
        }
    }

    fn paint(&mut self, ctx: &mut dyn PaintContext) {
        let panes = self.get_panes_to_render();
        if panes.is_empty() {
            ctx.clear(Color::rgb(0, 0, 0));
            return;
        }

        self.check_for_config_reload();
        self.update_title();

        let start = std::time::Instant::now();
        for pos in panes {
            if pos.is_active {
                self.update_text_cursor(&pos.pane);
            }
            if let Err(err) = self.paint_tab(&pos, ctx) {
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
        }
        log::debug!("paint_tab elapsed={:?}", start.elapsed());
        metrics::value!("gui.paint.software", start.elapsed());
    }

    fn opengl_context_lost(&mut self, prior_window: &dyn WindowOps) -> anyhow::Result<()> {
        log::error!("context was lost, set up a new window");

        let render_state = RenderState::Software(SoftwareRenderState::new(
            &self.fonts,
            &self.render_metrics,
            ATLAS_SIZE,
        )?);

        let clipboard_contents = Arc::clone(&self.clipboard_contents);
        let dimensions = self.dimensions.clone();
        let mux_window_id = self.mux_window_id;

        let window = Window::new_window(
            "org.wezfurlong.wezterm",
            "wezterm",
            dimensions.pixel_width,
            dimensions.pixel_height,
            Box::new(Self {
                window: None,
                focused: None,
                mux_window_id,
                fonts: Rc::clone(&self.fonts),
                render_metrics: self.render_metrics.clone(),
                dimensions,
                terminal_size: self.terminal_size.clone(),
                render_state,
                input_map: InputMap::new(),
                leader_is_down: None,
                show_tab_bar: self.show_tab_bar,
                show_scroll_bar: self.show_scroll_bar,
                tab_bar: self.tab_bar.clone(),
                last_mouse_coords: self.last_mouse_coords.clone(),
                last_mouse_terminal_coords: self.last_mouse_terminal_coords.clone(),
                scroll_drag_start: self.scroll_drag_start.clone(),
                split_drag_start: self.split_drag_start.clone(),
                config_generation: self.config_generation,
                prev_cursor: self.prev_cursor.clone(),
                last_scroll_info: self.last_scroll_info.clone(),
                clipboard_contents: Arc::clone(&clipboard_contents),
                tab_state: RefCell::new(self.tab_state.borrow().clone()),
                pane_state: RefCell::new(self.pane_state.borrow().clone()),
                current_mouse_button: self.current_mouse_button.clone(),
                last_mouse_click: self.last_mouse_click.clone(),
                current_highlight: self.current_highlight.clone(),
                shape_cache: RefCell::new(LruCache::new(65536)),
                last_blink_paint: Instant::now(),
            }),
        )?;

        Self::apply_icon(&window)?;
        Self::start_periodic_maintenance(window.clone());
        Self::setup_clipboard(&window, mux_window_id, clipboard_contents);

        prior_window.close();
        window.enable_opengl();

        Ok(())
    }

    fn opengl_initialize(
        &mut self,
        window: &dyn WindowOps,
        maybe_ctx: anyhow::Result<std::rc::Rc<glium::backend::Context>>,
    ) -> anyhow::Result<()> {
        self.render_state = RenderState::Software(SoftwareRenderState::new(
            &self.fonts,
            &self.render_metrics,
            ATLAS_SIZE,
        )?);

        match maybe_ctx {
            Ok(ctx) => {
                match OpenGLRenderState::new(
                    ctx,
                    &self.fonts,
                    &self.render_metrics,
                    ATLAS_SIZE,
                    self.dimensions.pixel_width,
                    self.dimensions.pixel_height,
                ) {
                    Ok(gl) => {
                        log::info!(
                            "OpenGL initialized! {} {} is_context_loss_possible={}",
                            gl.context.get_opengl_renderer_string(),
                            gl.context.get_opengl_version_string(),
                            gl.context.is_context_loss_possible(),
                        );
                        self.render_state = RenderState::GL(gl);
                    }
                    Err(err) => {
                        log::error!("failed to create OpenGLRenderState: {}", err);
                    }
                }
            }
            Err(err) => {
                log::error!("OpenGL init failed: {}", err);
            }
        };

        window.show();

        match &self.render_state {
            RenderState::Software(_) => Err(anyhow::anyhow!("Falling back to software renderer")),
            RenderState::GL(_) => Ok(()),
        }
    }

    fn paint_opengl(&mut self, frame: &mut glium::Frame) {
        let panes = self.get_panes_to_render();
        if panes.is_empty() {
            frame.clear_color_srgb(0., 0., 0., 1.);
            return;
        }

        self.check_for_config_reload();
        let start = std::time::Instant::now();

        if let Some(pane) = self.get_active_pane_or_overlay() {
            let splits = self.get_splits();
            for split in &splits {
                self.paint_split_opengl(split, &pane).ok();
            }
        }

        for pos in panes {
            if pos.is_active {
                self.update_text_cursor(&pos.pane);
            }
            if let Err(err) = self.paint_tab_opengl(&pos, frame) {
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
        }

        self.call_draw(frame).ok();
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
        tab: &Rc<Tab>,
        mux_window_id: MuxWindowId,
    ) -> anyhow::Result<()> {
        let size = tab.get_size();
        let physical_rows = size.rows as usize;
        let physical_cols = size.cols as usize;

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

        let render_state = RenderState::Software(SoftwareRenderState::new(
            fontconfig,
            &render_metrics,
            ATLAS_SIZE,
        )?);

        let clipboard_contents = Arc::new(Mutex::new(None));

        let window = Window::new_window(
            "org.wezfurlong.wezterm",
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
                input_map: InputMap::new(),
                leader_is_down: None,
                show_tab_bar,
                show_scroll_bar: config.enable_scroll_bar,
                tab_bar: TabBarState::default(),
                last_mouse_coords: (0, -1),
                last_mouse_terminal_coords: (0, 0),
                scroll_drag_start: None,
                split_drag_start: None,
                config_generation: config.generation(),
                prev_cursor: PrevCursorPos::new(),
                last_scroll_info: RenderableDimensions::default(),
                clipboard_contents: Arc::clone(&clipboard_contents),
                tab_state: RefCell::new(HashMap::new()),
                pane_state: RefCell::new(HashMap::new()),
                current_mouse_button: None,
                last_mouse_click: None,
                current_highlight: None,
                shape_cache: RefCell::new(LruCache::new(65536)),
                last_blink_paint: Instant::now(),
            }),
        )?;

        Self::apply_icon(&window)?;
        Self::start_periodic_maintenance(window.clone());
        Self::setup_clipboard(&window, mux_window_id, clipboard_contents);

        if super::is_opengl_enabled() {
            window.enable_opengl();
        } else {
            window.show();
        }

        crate::update::start_update_checker();
        Ok(())
    }

    fn setup_clipboard(
        window: &Window,
        mux_window_id: MuxWindowId,
        clipboard_contents: Arc<Mutex<Option<String>>>,
    ) {
        let clipboard: Arc<dyn wezterm_term::Clipboard> = Arc::new(ClipboardHelper {
            window: window.clone(),
            clipboard_contents,
        });
        let mux = Mux::get().unwrap();

        let mut mux_window = mux.get_window_mut(mux_window_id).unwrap();

        mux_window.set_clipboard(&clipboard);
        for tab in mux_window.iter() {
            for pane in tab.get_active_pane() {
                pane.set_clipboard(&clipboard);
            }
        }
    }

    fn apply_icon(window: &Window) -> anyhow::Result<()> {
        let icon_image =
            image::load_from_memory(include_bytes!("../../../assets/icon/terminal.png"))?;
        let image = icon_image.to_bgra();
        let (width, height) = image.dimensions();
        window.set_icon(Image::from_raw(
            width as usize,
            height as usize,
            image.into_raw(),
        ));
        Ok(())
    }

    fn start_periodic_maintenance(window: Window) {
        Connection::get().unwrap().schedule_timer(
            std::time::Duration::from_millis(35),
            move || {
                window.apply(move |myself, window| {
                    if let Some(myself) = myself.downcast_mut::<Self>() {
                        myself.periodic_window_maintenance(window)?;
                    }
                    Ok(())
                });
            },
        );
    }

    fn periodic_window_maintenance(&mut self, _window: &dyn WindowOps) -> anyhow::Result<()> {
        let mux = Mux::get().unwrap();

        if let Some(pane) = self.get_active_pane_or_overlay() {
            let mut needs_invalidate = false;

            // If the config was reloaded, ask the window to apply
            // and render any changes
            self.check_for_config_reload();

            let config = configuration();

            let render = pane.renderer();

            // If blinking is permitted, and the cursor shape is set
            // to a blinking variant, and it's been longer than the
            // blink rate interval, then invalidate and redraw
            // so that we will re-evaluate the cursor visibility.
            // This is pretty heavyweight: it would be nice to only invalidate
            // the line on which the cursor resides, and then only if the cursor
            // is within the viewport.
            if config.cursor_blink_rate != 0 && self.focused.is_some() {
                let shape = config
                    .default_cursor_style
                    .effective_shape(render.get_cursor_position().shape);
                if shape.is_blinking() {
                    let now = Instant::now();
                    if now.duration_since(self.last_blink_paint)
                        > Duration::from_millis(config.cursor_blink_rate)
                    {
                        needs_invalidate = true;
                        self.last_blink_paint = now;
                    }
                }
            }

            // If the model is dirty, arrange to re-paint
            let dims = render.get_dimensions();
            let viewport = self
                .get_viewport(pane.pane_id())
                .unwrap_or(dims.physical_top);
            let visible_range = viewport..viewport + dims.viewport_rows as StableRowIndex;
            let dirty = render.get_dirty_lines(visible_range);

            if !dirty.is_empty() {
                if pane.downcast_ref::<SearchOverlay>().is_none()
                    && pane.downcast_ref::<CopyOverlay>().is_none()
                {
                    // If any of the changed lines intersect with the
                    // selection, then we need to clear the selection, but not
                    // when the search overlay is active; the search overlay
                    // marks lines as dirty to force invalidate them for
                    // highlighting purpose but also manipulates the selection
                    // and we want to allow it to retain the selection it made!

                    let clear_selection = if let Some(selection_range) =
                        self.selection(pane.pane_id()).range.as_ref()
                    {
                        let selection_rows = selection_range.rows();
                        selection_rows.into_iter().any(|row| dirty.contains(row))
                    } else {
                        false
                    };

                    if clear_selection {
                        self.selection(pane.pane_id()).range.take();
                        self.selection(pane.pane_id()).start.take();
                    }
                }

                needs_invalidate = true;
            }

            if let Some(mut mux_window) = mux.get_window_mut(self.mux_window_id) {
                if mux_window.check_and_reset_invalidated() {
                    needs_invalidate = true;
                }
            }

            if needs_invalidate {
                self.window.as_ref().unwrap().invalidate();
            }
        } else {
            self.window.as_ref().unwrap().close();
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
        if window.len() == 1 {
            self.show_tab_bar = config.enable_tab_bar && !config.hide_tab_bar_if_only_one_tab;
        } else {
            self.show_tab_bar = config.enable_tab_bar;
        }

        self.show_scroll_bar = config.enable_scroll_bar;
        self.shape_cache.borrow_mut().clear();
        self.input_map = InputMap::new();
        self.leader_is_down = None;
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

        let tab = match self.get_active_pane_or_overlay() {
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
            &config,
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
        drop(window);

        let title = if let Some(pane) = self.get_active_pane_or_overlay() {
            pane.get_title()
        } else {
            return;
        };

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

    fn update_text_cursor(&mut self, pane: &Rc<dyn Pane>) {
        let term = pane.renderer();
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

    fn activate_tab(&mut self, tab_idx: isize) -> anyhow::Result<()> {
        if let Some(tab) = self.get_active_pane_or_overlay() {
            tab.focus_changed(false);
        }

        let mux = Mux::get().unwrap();
        let mut window = mux
            .get_window_mut(self.mux_window_id)
            .ok_or_else(|| anyhow!("no such window"))?;

        let max = window.len();

        let tab_idx = if tab_idx < 0 {
            max.saturating_sub(tab_idx.abs() as usize)
        } else {
            tab_idx as usize
        };

        if tab_idx < max {
            window.set_active(tab_idx);

            drop(window);

            if let Some(tab) = self.get_active_pane_or_overlay() {
                tab.focus_changed(true);
            }

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
        self.activate_tab((tab as usize % max) as isize)
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
        let tabs: Vec<(String, TabId, usize)> = window
            .iter()
            .map(|tab| {
                (
                    tab.get_active_pane()
                        .expect("tab to have a pane")
                        .get_title(),
                    tab.tab_id(),
                    tab.count_panes(),
                )
            })
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

        let domain_id_of_current_pane = tab
            .get_active_pane()
            .expect("tab has no panes!")
            .domain_id();
        let size = self.terminal_size;

        let (overlay, future) = start_overlay(self, &tab, move |tab_id, term| {
            launcher(
                tab_id,
                domain_id_of_current_pane,
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
        let pane = match self.get_active_pane_or_overlay() {
            Some(pane) => pane,
            None => return Ok(()),
        };
        let render = pane.renderer();
        let dims = render.get_dimensions();
        let position = self
            .get_viewport(pane.pane_id())
            .unwrap_or(dims.physical_top)
            .saturating_add(amount * dims.viewport_rows as isize);
        drop(render);
        self.set_viewport(pane.pane_id(), Some(position), dims);
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

    fn spawn_command(&mut self, spawn: &SpawnCommand, spawn_where: SpawnWhere) {
        Self::spawn_command_impl(
            spawn,
            spawn_where,
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
        spawn_where: SpawnWhere,
        size: PtySize,
        mux_window_id: MuxWindowId,
        clipboard: ClipboardHelper,
    ) {
        let spawn = spawn.clone();

        promise::spawn::spawn(async move {
            let mux = Mux::get().unwrap();
            let activity = Activity::new();

            let mux_window_id = if spawn_where == SpawnWhere::NewWindow {
                mux.new_empty_window()
            } else {
                mux_window_id
            };

            let (domain, cwd) = match spawn.domain {
                SpawnTabDomain::DefaultDomain => {
                    let cwd = mux
                        .get_active_tab_for_window(mux_window_id)
                        .and_then(|tab| tab.get_active_pane())
                        .and_then(|pane| pane.get_current_working_dir());
                    (mux.default_domain().clone(), cwd)
                }
                SpawnTabDomain::CurrentPaneDomain => {
                    if spawn_where == SpawnWhere::NewWindow {
                        // CurrentPaneDomain is the default value for the spawn domain.
                        // It doesn't make sense to use it when spawning a new window,
                        // so we treat it as DefaultDomain instead.
                        let cwd = mux
                            .get_active_tab_for_window(mux_window_id)
                            .and_then(|tab| tab.get_active_pane())
                            .and_then(|pane| pane.get_current_working_dir());
                        (mux.default_domain().clone(), cwd)
                    } else {
                        let tab = match mux.get_active_tab_for_window(mux_window_id) {
                            Some(tab) => tab,
                            None => bail!("window has no tabs?"),
                        };
                        let pane = tab
                            .get_active_pane()
                            .ok_or_else(|| anyhow!("current tab has no pane!?"))?;
                        (
                            mux.get_domain(pane.domain_id()).ok_or_else(|| {
                                anyhow!("current tab has unresolvable domain id!?")
                            })?,
                            pane.get_current_working_dir(),
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

            match spawn_where {
                SpawnWhere::SplitPane(direction) => {
                    let mux = Mux::get().unwrap();
                    if let Some(tab) = mux.get_active_tab_for_window(mux_window_id) {
                        let pane = tab
                            .get_active_pane()
                            .ok_or_else(|| anyhow!("tab to have a pane"))?;

                        log::error!("doing split_pane");
                        domain
                            .split_pane(cmd_builder, cwd, tab.tab_id(), pane.pane_id(), direction)
                            .await?;
                    } else {
                        log::error!("boop");
                    }
                }
                _ => {
                    let tab = domain.spawn(size, cmd_builder, cwd, mux_window_id).await?;
                    let tab_id = tab.tab_id();
                    let pane = tab
                        .get_active_pane()
                        .ok_or_else(|| anyhow!("newly spawned tab to have a pane"))?;

                    if spawn_where == SpawnWhere::NewWindow {
                        let front_end = front_end().expect("to be called on gui thread");
                        let fonts = Rc::new(FontConfiguration::new());
                        front_end.spawn_new_window(&fonts, &tab, mux_window_id)?;
                    } else {
                        let clipboard: Arc<dyn wezterm_term::Clipboard> = Arc::new(clipboard);
                        pane.set_clipboard(&clipboard);
                        let mut window = mux
                            .get_window_mut(mux_window_id)
                            .ok_or_else(|| anyhow!("no such window!?"))?;
                        if let Some(idx) = window.idx_by_id(tab_id) {
                            window.set_active(idx);
                        }
                    }
                }
            };

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
            SpawnWhere::NewTab,
        );
    }

    fn selection_text(&self, pane: &Rc<dyn Pane>) -> String {
        let mut s = String::new();
        if let Some(sel) = self
            .selection(pane.pane_id())
            .range
            .as_ref()
            .map(|r| r.normalize())
        {
            let mut last_was_wrapped = false;
            let mut renderer = pane.renderer();
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

    fn paste_from_clipboard(&mut self, pane: &Rc<dyn Pane>, clipboard: Clipboard) {
        let pane_id = pane.pane_id();
        let window = self.window.as_ref().unwrap().clone();
        let future = window.get_clipboard(clipboard);
        promise::spawn::spawn(async move {
            if let Ok(clip) = future.await {
                window.apply(move |term_window, _window| {
                    let clip = clip.clone();
                    if let Some(term_window) = term_window.downcast_mut::<TermWindow>() {
                        if let Some(pane) =
                            term_window.pane_state(pane_id).overlay.clone().or_else(|| {
                                let mux = Mux::get().unwrap();
                                mux.get_pane(pane_id)
                            })
                        {
                            pane.trickle_paste(clip).ok();
                        }
                    }
                    Ok(())
                });
            }
        });
    }

    fn perform_key_assignment(
        &mut self,
        pane: &Rc<dyn Pane>,
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
                self.spawn_command(spawn, SpawnWhere::NewTab);
            }
            SpawnCommandInNewWindow(spawn) => {
                self.spawn_command(spawn, SpawnWhere::NewWindow);
            }
            SplitHorizontal(spawn) => {
                log::error!("SplitHorizontal {:?}", spawn);
                self.spawn_command(spawn, SpawnWhere::SplitPane(SplitDirection::Horizontal));
            }
            SplitVertical(spawn) => {
                log::error!("SplitVertical {:?}", spawn);
                self.spawn_command(spawn, SpawnWhere::SplitPane(SplitDirection::Vertical));
            }
            ToggleFullScreen => {
                // self.toggle_full_screen(),
            }
            Copy => {
                self.window
                    .as_ref()
                    .unwrap()
                    .set_clipboard(self.selection_text(pane));
            }
            Paste => {
                self.paste_from_clipboard(pane, Clipboard::default());
            }
            PastePrimarySelection => {
                self.paste_from_clipboard(pane, Clipboard::PrimarySelection);
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
            SendString(s) => pane.writer().write_all(s.as_bytes())?,
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
            CloseCurrentPane => self.close_active_pane(),
            Nop | DisableDefaultAssignment => {}
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
            SelectTextAtMouseCursor(mode) => self.select_text_at_mouse_cursor(*mode, pane),
            ExtendSelectionToMouseCursor(mode) => {
                self.extend_selection_at_mouse_cursor(*mode, pane)
            }
            OpenLinkAtMouseCursor => {
                // They clicked on a link, so let's open it!
                // Ensure that we spawn the `open` call outside of the context
                // of our window loop; on Windows it can cause a panic due to
                // triggering our WndProc recursively.
                if let Some(link) = self.current_highlight.as_ref().cloned() {
                    promise::spawn::spawn(async move {
                        log::error!("clicking {}", link.uri());
                        if let Err(err) = open::that(link.uri()) {
                            log::error!("failed to open {}: {:?}", link.uri(), err);
                        }
                    });
                }
            }
            CompleteSelectionOrOpenLinkAtMouseCursor => {
                let text = self.selection_text(pane);
                if !text.is_empty() {
                    let window = self.window.as_ref().unwrap();
                    window.set_clipboard(text);
                    window.invalidate();
                } else {
                    return self
                        .perform_key_assignment(pane, &KeyAssignment::OpenLinkAtMouseCursor);
                }
            }
            CompleteSelection => {
                let text = self.selection_text(pane);
                if !text.is_empty() {
                    let window = self.window.as_ref().unwrap();
                    window.set_clipboard(text);
                    window.invalidate();
                }
            }
            ClearScrollback => {
                pane.erase_scrollback();
                let window = self.window.as_ref().unwrap();
                window.invalidate();
            }
            Search(pattern) => {
                if let Some(pane) = self.get_active_pane_no_overlay() {
                    let search = SearchOverlay::with_pane(self, &pane, pattern.clone());
                    self.assign_overlay_for_pane(pane.pane_id(), search);
                }
            }
            ActivateCopyMode => {
                if let Some(pane) = self.get_active_pane_no_overlay() {
                    let copy = CopyOverlay::with_pane(self, &pane);
                    self.assign_overlay_for_pane(pane.pane_id(), copy);
                }
            }
            AdjustPaneSize(direction, amount) => {
                let mux = Mux::get().unwrap();
                let tab = match mux.get_active_tab_for_window(self.mux_window_id) {
                    Some(tab) => tab,
                    None => return Ok(()),
                };

                let tab_id = tab.tab_id();

                if self.tab_state(tab_id).overlay.is_none() {
                    tab.adjust_pane_size(*direction, *amount);
                }
            }
            ActivatePaneDirection(direction) => {
                let mux = Mux::get().unwrap();
                let tab = match mux.get_active_tab_for_window(self.mux_window_id) {
                    Some(tab) => tab,
                    None => return Ok(()),
                };

                let tab_id = tab.tab_id();

                if self.tab_state(tab_id).overlay.is_none() {
                    tab.activate_pane_direction(*direction);
                }
            }
            TogglePaneZoomState => {
                let mux = Mux::get().unwrap();
                let tab = match mux.get_active_tab_for_window(self.mux_window_id) {
                    Some(tab) => tab,
                    None => return Ok(()),
                };
                tab.toggle_zoom();
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
        mut scale_changed_cells: Option<RowsAndCols>,
    ) {
        let orig_dimensions = self.dimensions;

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

        if let Err(err) = self.render_state.advise_of_window_size_change(
            &self.render_metrics,
            dimensions.pixel_width,
            dimensions.pixel_height,
        ) {
            log::error!(
                "failed to advise of resize from {:?} -> {:?}: {:?}",
                orig_dimensions,
                dimensions,
                err
            );
            // Try to restore the original dimensions
            self.dimensions = orig_dimensions;
            // Avoid the inner resize below
            scale_changed_cells.take();
        } else {
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

    fn close_active_pane(&mut self) {
        let mux = Mux::get().unwrap();
        let tab = match mux.get_active_tab_for_window(self.mux_window_id) {
            Some(tab) => tab,
            None => return,
        };
        tab.kill_active_pane();
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

    fn paint_tab(
        &mut self,
        pos: &PositionedPane,
        ctx: &mut dyn PaintContext,
    ) -> anyhow::Result<()> {
        let palette = pos.pane.palette();
        let first_line_offset = if self.show_tab_bar { 1 } else { 0 };

        let mut term = pos.pane.renderer();
        let cursor = term.get_cursor_position();
        if pos.is_active {
            self.prev_cursor.update(&cursor);
        }
        let current_viewport = self.get_viewport(pos.pane.pane_id());

        let dims = term.get_dimensions();

        if self.show_tab_bar && pos.index == 0 {
            self.render_screen_line(
                ctx,
                0,
                None,
                self.tab_bar.line(),
                0..0,
                &cursor,
                &palette,
                &dims,
                pos,
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
                    .selection(pos.pane.pane_id())
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
                    &palette,
                    &dims,
                    pos,
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
            // FIXME: scrollbar for active pane
            let current_viewport = self.get_viewport(pos.pane.pane_id());
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

    fn paint_split_opengl(
        &mut self,
        split: &PositionedSplit,
        pane: &Rc<dyn Pane>,
    ) -> anyhow::Result<()> {
        let gl_state = self.render_state.opengl();
        let mut vb = gl_state.glyph_vertex_buffer.borrow_mut();
        let mut quads = gl_state.quads.map(&mut vb);
        let config = configuration();
        let text = if split.direction == SplitDirection::Horizontal {
            "│"
        } else {
            "─"
        };
        let palette = pane.palette();
        let foreground = rgbcolor_to_window_color(palette.foreground);
        let background = rgbcolor_to_window_color(palette.background);

        let style = self.fonts.match_style(&config, &CellAttributes::default());
        let glyph_info = {
            let key = BorrowedShapeCacheKey { style, text };
            match self.lookup_cached_shape(&key) {
                Some(Ok(info)) => info,
                Some(Err(err)) => return Err(err),
                None => {
                    let font = self.fonts.resolve_font(style)?;
                    match font.shape(text) {
                        Ok(info) => {
                            self.shape_cache
                                .borrow_mut()
                                .put(key.to_owned(), Ok(Rc::new(info)));
                            self.lookup_cached_shape(&key).unwrap().unwrap()
                        }
                        Err(err) => {
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
            let glyph = gl_state
                .glyph_cache
                .borrow_mut()
                .cached_glyph(info, style)?;

            let left = (glyph.x_offset + glyph.bearing_x).get() as f32;
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
                    num_cells: info.num_cells as usize,
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
                quad.set_bg_color(background);
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

    fn paint_tab_opengl(
        &mut self,
        pos: &PositionedPane,
        frame: &mut glium::Frame,
    ) -> anyhow::Result<()> {
        let palette = pos.pane.palette();

        let background_color = palette.resolve_bg(wezterm_term::color::ColorAttribute::Default);
        if pos.index == 0 {
            let (r, g, b, a) = background_color.to_tuple_rgba();
            frame.clear_color_srgb(r, g, b, a);
        }

        let first_line_offset = if self.show_tab_bar { 1 } else { 0 };

        let mut term = pos.pane.renderer();
        let cursor = term.get_cursor_position();
        if pos.is_active {
            self.prev_cursor.update(&cursor);
        }

        let current_viewport = self.get_viewport(pos.pane.pane_id());
        let (stable_top, lines);
        let dims = term.get_dimensions();
        let config = configuration();

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

        let cursor_border_color = rgbcolor_to_window_color(palette.cursor_border);
        let foreground = rgbcolor_to_window_color(palette.foreground);
        let background = rgbcolor_to_window_color(palette.background);

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
                    background,
                    pos,
                },
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

        let selrange = self.selection(pos.pane.pane_id()).range.clone();

        for (line_idx, line) in lines.iter().enumerate() {
            let stable_row = stable_top + line_idx as StableRowIndex;
            let selrange = selrange
                .map(|sel| sel.cols_for_row(stable_row))
                .unwrap_or(0..0);

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
                    background,
                    pos,
                },
                &mut quads,
            )?;
        }

        Ok(())
    }

    fn call_draw(&mut self, frame: &mut glium::Frame) -> anyhow::Result<()> {
        let gl_state = self.render_state.opengl();
        let vb = gl_state.glyph_vertex_buffer.borrow_mut();

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

    fn lookup_cached_shape(
        &self,
        key: &dyn ShapeCacheKeyTrait,
    ) -> Option<anyhow::Result<Rc<Vec<GlyphInfo>>>> {
        match self.shape_cache.borrow_mut().get(key) {
            Some(Ok(info)) => Some(Ok(Rc::clone(info))),
            Some(Err(err)) => Some(Err(anyhow!("cached shaper error: {}", err))),
            None => None,
        }
    }

    /// "Render" a line of the terminal screen into the vertex buffer.
    /// This is nominally a matter of setting the fg/bg color and the
    /// texture coordinates for a given glyph.  There's a little bit
    /// of extra complexity to deal with multi-cell glyphs.
    fn render_screen_line_opengl(
        &self,
        params: RenderScreenLineOpenGLParams,
        quads: &mut MappedQuads,
    ) -> anyhow::Result<()> {
        let gl_state = self.render_state.opengl();

        let num_cols = params.dims.cols;

        // Break the line into clusters of cells with the same attributes
        let cell_clusters = params.line.cluster();
        let mut last_cell_idx = 0;
        for cluster in cell_clusters {
            let attrs = &cluster.attrs;
            let is_highlited_hyperlink = match (&attrs.hyperlink, &self.current_highlight) {
                (&Some(ref this), &Some(ref highlight)) => Arc::ptr_eq(this, highlight),
                _ => false,
            };
            let style = self.fonts.match_style(params.config, attrs);

            let bg_color = params.palette.resolve_bg(attrs.background);
            let fg_color = match attrs.foreground {
                wezterm_term::color::ColorAttribute::Default => {
                    if let Some(fg) = style.foreground {
                        fg
                    } else {
                        params.palette.resolve_fg(attrs.foreground)
                    }
                }
                wezterm_term::color::ColorAttribute::PaletteIndex(idx)
                    if idx < 8 && params.config.bold_brightens_ansi_colors =>
                {
                    // For compatibility purposes, switch to a brighter version
                    // of one of the standard ANSI colors when Bold is enabled.
                    // This lifts black to dark grey.
                    let idx = if attrs.intensity() == wezterm_term::Intensity::Bold {
                        idx + 8
                    } else {
                        idx
                    };
                    params
                        .palette
                        .resolve_fg(wezterm_term::color::ColorAttribute::PaletteIndex(idx))
                }
                _ => params.palette.resolve_fg(attrs.foreground),
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
                let key = BorrowedShapeCacheKey {
                    style,
                    text: &cluster.text,
                };
                match self.lookup_cached_shape(&key) {
                    Some(Ok(info)) => info,
                    Some(Err(err)) => return Err(err),
                    None => {
                        let font = self.fonts.resolve_font(style)?;
                        match font.shape(&cluster.text) {
                            Ok(info) => {
                                self.shape_cache
                                    .borrow_mut()
                                    .put(key.to_owned(), Ok(Rc::new(info)));
                                self.lookup_cached_shape(&key).unwrap().unwrap()
                            }
                            Err(err) => {
                                let res = anyhow!("shaper error: {}", err);
                                self.shape_cache.borrow_mut().put(key.to_owned(), Err(err));
                                return Err(res);
                            }
                        }
                    }
                }
            };

            for info in glyph_info.iter() {
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
                        attrs.overline(),
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
                        params.stable_line_idx,
                        cell_idx,
                        params.cursor,
                        &params.selection,
                        glyph_color,
                        bg_color,
                        params.palette,
                        params.pos.is_active,
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

                            let mut quad = match quads.cell(cell_idx, params.line_idx) {
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
                            quad.set_cursor_color(params.cursor_border_color);

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

        let white_space = gl_state.util_sprites.white_space.texture_coords();

        for cell_idx in last_cell_idx + 1..num_cols {
            // Even though we don't have a cell for these, they still
            // hold the cursor or the selection so we need to compute
            // the colors in the usual way.
            let (glyph_color, bg_color, cursor_shape) = self.compute_cell_fg_bg(
                params.stable_line_idx,
                cell_idx,
                params.cursor,
                &params.selection,
                params.foreground,
                params.background,
                params.palette,
                params.pos.is_active,
            );

            let mut quad =
                match quads.cell(cell_idx + params.pos.left, params.line_idx + params.pos.top) {
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
            quad.set_cursor_color(params.cursor_border_color);
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
        palette: &ColorPalette,
        dims: &RenderableDimensions,
        pos: &PositionedPane,
    ) -> anyhow::Result<()> {
        let config = configuration();

        let padding_left = config.window_padding.left as isize;
        let padding_top = config.window_padding.top as isize;

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
                wezterm_term::color::ColorAttribute::Default => {
                    if let Some(fg) = style.foreground {
                        fg
                    } else {
                        palette.resolve_fg(attrs.foreground)
                    }
                }
                wezterm_term::color::ColorAttribute::PaletteIndex(idx)
                    if idx < 8 && config.bold_brightens_ansi_colors =>
                {
                    // For compatibility purposes, switch to a brighter version
                    // of one of the standard ANSI colors when Bold is enabled.
                    // This lifts black to dark grey.
                    let idx = if attrs.intensity() == wezterm_term::Intensity::Bold {
                        idx + 8
                    } else {
                        idx
                    };
                    palette.resolve_fg(wezterm_term::color::ColorAttribute::PaletteIndex(idx))
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
                        pos.is_active,
                    );

                    let cell_rect = Rect::new(
                        Point::new(
                            ((pos.left + cell_idx) as isize * self.render_metrics.cell_size.width)
                                + padding_left,
                            (self.render_metrics.cell_size.height * (pos.top + line_idx) as isize)
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
                            attrs.overline(),
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

                    if cursor_shape.is_some() {
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
                pos.is_active,
            );

            let cell_rect = Rect::new(
                Point::new(
                    (pos.left + cell_idx) as isize * self.render_metrics.cell_size.width,
                    self.render_metrics.cell_size.height * (pos.top + line_idx) as isize,
                ),
                self.render_metrics.cell_size,
            );
            ctx.clear_rect(cell_rect, bg_color);

            if cursor_shape.is_some() {
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
        is_active_pane: bool,
    ) -> (Color, Color, Option<CursorShape>) {
        let selected = selection.contains(&cell_idx);

        let is_cursor = stable_line_idx == Some(cursor.y) && cursor.x == cell_idx;

        let (cursor_shape, visibility) =
            if is_cursor && cursor.visibility == CursorVisibility::Visible {
                // This logic figures out whether the cursor is visible or not.
                // If the cursor is explicitly hidden then it is obviously not
                // visible.
                // If the cursor is set to a blinking mode then we are visible
                // depending on the current time.
                let config = configuration();
                let shape = config.default_cursor_style.effective_shape(cursor.shape);
                // Work out the blinking shape if its a blinking cursor and it hasn't been disabled
                // and the window is focused.
                let blinking = is_active_pane
                    && shape.is_blinking()
                    && config.cursor_blink_rate != 0
                    && self.focused.is_some();
                if blinking {
                    // Divide the time since we last moved by the blink rate.
                    // If the result is even then the cursor is "on", else it
                    // is "off"
                    let now = std::time::Instant::now();
                    let milli_uptime = now
                        .duration_since(self.prev_cursor.last_cursor_movement())
                        .as_millis();
                    let ticks = milli_uptime / config.cursor_blink_rate as u128;
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
                (cursor.shape, CursorVisibility::Hidden)
            };

        let (fg_color, bg_color) = match (
            selected,
            self.focused.is_some() && is_active_pane,
            cursor_shape,
            visibility,
        ) {
            // Selected text overrides colors
            (true, _, _, CursorVisibility::Hidden) => (
                rgbcolor_to_window_color(palette.selection_fg),
                rgbcolor_to_window_color(palette.selection_bg),
            ),
            // Cursor cell overrides colors
            (_, true, CursorShape::BlinkingBlock, CursorVisibility::Visible)
            | (_, true, CursorShape::SteadyBlock, CursorVisibility::Visible) => (
                rgbcolor_to_window_color(palette.cursor_fg),
                rgbcolor_to_window_color(palette.cursor_bg),
            ),
            // Normally, render the cell as configured (or if the window is unfocused)
            _ => (fg_color, bg_color),
        };

        (
            fg_color,
            bg_color,
            if visibility == CursorVisibility::Visible {
                Some(cursor_shape)
            } else {
                None
            },
        )
    }

    pub fn pane_state(&self, pane_id: PaneId) -> RefMut<PaneState> {
        RefMut::map(self.pane_state.borrow_mut(), |state| {
            state.entry(pane_id).or_insert_with(PaneState::default)
        })
    }

    pub fn tab_state(&self, tab_id: TabId) -> RefMut<TabState> {
        RefMut::map(self.tab_state.borrow_mut(), |state| {
            state.entry(tab_id).or_insert_with(TabState::default)
        })
    }

    pub fn selection(&self, pane_id: PaneId) -> RefMut<Selection> {
        RefMut::map(self.pane_state(pane_id), |state| &mut state.selection)
    }

    pub fn get_viewport(&self, pane_id: PaneId) -> Option<StableRowIndex> {
        self.pane_state(pane_id).viewport
    }

    pub fn set_viewport(
        &mut self,
        pane_id: PaneId,
        position: Option<StableRowIndex>,
        dims: RenderableDimensions,
    ) {
        let pos = match position {
            Some(pos) => {
                // Drop out of scrolling mode if we're off the bottom
                if pos >= dims.physical_top {
                    None
                } else {
                    Some(pos.max(dims.scrollback_top))
                }
            }
            None => None,
        };

        let mut state = self.pane_state(pane_id);
        if pos != state.viewport {
            state.viewport = pos;

            // This is a bit gross.  If we add other overlays that need this information,
            // this should get extracted out into a trait
            if let Some(overlay) = state.overlay.as_ref() {
                if let Some(search_overlay) = overlay.downcast_ref::<SearchOverlay>() {
                    search_overlay.viewport_changed(pos);
                } else if let Some(copy) = overlay.downcast_ref::<CopyOverlay>() {
                    copy.viewport_changed(pos);
                }
            }
            self.window.as_ref().unwrap().invalidate();
        }
    }

    fn mouse_event_tab_bar(&mut self, x: usize, event: &MouseEvent, context: &dyn WindowOps) {
        match event.kind {
            WMEK::Press(MousePress::Left) => match self.tab_bar.hit_test(x) {
                TabBarItem::Tab(tab_idx) => {
                    self.activate_tab(tab_idx as isize).ok();
                }
                TabBarItem::NewTabButton => {
                    self.spawn_tab(&SpawnTabDomain::CurrentPaneDomain);
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
        pane: Rc<dyn Pane>,
        event: &MouseEvent,
        context: &dyn WindowOps,
    ) {
        if let WMEK::Press(MousePress::Left) = event.kind {
            let render = pane.renderer();
            let dims = render.get_dimensions();
            let current_viewport = self.get_viewport(pane.pane_id());

            let hit_result = ScrollHit::test(
                event.coords.y,
                &*render,
                current_viewport,
                self.terminal_size,
                &self.dimensions,
            );
            drop(render);

            match hit_result {
                ScrollHit::Above => {
                    // Page up
                    self.set_viewport(
                        pane.pane_id(),
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
                        pane.pane_id(),
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

    fn extend_selection_at_mouse_cursor(
        &mut self,
        mode: Option<SelectionMode>,
        pane: &Rc<dyn Pane>,
    ) {
        let mode = mode.unwrap_or(SelectionMode::Cell);
        let (x, y) = self.last_mouse_terminal_coords;
        match mode {
            SelectionMode::Cell => {
                let end = SelectionCoordinate { x, y };
                let selection_range = self.selection(pane.pane_id()).range.take();
                let sel = match selection_range {
                    None => {
                        SelectionRange::start(self.selection(pane.pane_id()).start.unwrap_or(end))
                            .extend(end)
                    }
                    Some(sel) => sel.extend(end),
                };
                self.selection(pane.pane_id()).range = Some(sel);
            }
            SelectionMode::Word => {
                let end_word = SelectionRange::word_around(
                    SelectionCoordinate { x, y },
                    &mut *pane.renderer(),
                );

                let start_coord = self
                    .selection(pane.pane_id())
                    .start
                    .clone()
                    .unwrap_or(end_word.start);
                let start_word = SelectionRange::word_around(start_coord, &mut *pane.renderer());

                let selection_range = start_word.extend_with(end_word);
                self.selection(pane.pane_id()).range = Some(selection_range);
            }
            SelectionMode::Line => {
                let end_line = SelectionRange::line_around(SelectionCoordinate { x, y });

                let start_coord = self
                    .selection(pane.pane_id())
                    .start
                    .clone()
                    .unwrap_or(end_line.start);
                let start_line = SelectionRange::line_around(start_coord);

                let selection_range = start_line.extend_with(end_line);
                self.selection(pane.pane_id()).range = Some(selection_range);
            }
        }

        // When the mouse gets close enough to the top or bottom then scroll
        // the viewport so that we can see more in that direction and are able
        // to select more than fits in the viewport.

        // This is similar to the logic in the copy mode overlay, but the gap
        // is smaller because it feels more natural for mouse selection to have
        // a smaller gpa.
        const VERTICAL_GAP: isize = 2;
        let dims = pane.renderer().get_dimensions();
        let top = self
            .get_viewport(pane.pane_id())
            .unwrap_or(dims.physical_top);
        let vertical_gap = if dims.physical_top <= VERTICAL_GAP {
            1
        } else {
            VERTICAL_GAP
        };
        let top_gap = y - top;
        if top_gap < vertical_gap {
            // Increase the gap so we can "look ahead"
            self.set_viewport(pane.pane_id(), Some(y.saturating_sub(vertical_gap)), dims);
        } else {
            let bottom_gap = (dims.viewport_rows as isize).saturating_sub(top_gap);
            if bottom_gap < vertical_gap {
                self.set_viewport(pane.pane_id(), Some(top + vertical_gap - bottom_gap), dims);
            }
        }

        self.window.as_ref().unwrap().invalidate();
    }

    fn select_text_at_mouse_cursor(&mut self, mode: SelectionMode, pane: &Rc<dyn Pane>) {
        let (x, y) = self.last_mouse_terminal_coords;
        match mode {
            SelectionMode::Line => {
                let start = SelectionCoordinate { x, y };
                let selection_range = SelectionRange::line_around(start);

                self.selection(pane.pane_id()).start = Some(start);
                self.selection(pane.pane_id()).range = Some(selection_range);
            }
            SelectionMode::Word => {
                let selection_range = SelectionRange::word_around(
                    SelectionCoordinate { x, y },
                    &mut *pane.renderer(),
                );

                self.selection(pane.pane_id()).start = Some(selection_range.start);
                self.selection(pane.pane_id()).range = Some(selection_range);
            }
            SelectionMode::Cell => {
                self.selection(pane.pane_id())
                    .begin(SelectionCoordinate { x, y });
            }
        }

        self.window.as_ref().unwrap().invalidate();
    }

    fn mouse_event_terminal(
        &mut self,
        mut pane: Rc<dyn Pane>,
        mut x: usize,
        mut y: i64,
        event: &MouseEvent,
        context: &dyn WindowOps,
    ) {
        let mut on_split = false;
        if y >= 0 {
            let y = y as usize;

            for split in self.get_splits() {
                on_split = match split.direction {
                    SplitDirection::Horizontal => {
                        if x == split.left && y >= split.top && y <= split.top + split.size {
                            true
                        } else {
                            false
                        }
                    }
                    SplitDirection::Vertical => {
                        if y == split.top && x >= split.left && x <= split.left + split.size {
                            true
                        } else {
                            false
                        }
                    }
                };

                if on_split && event.kind == WMEK::Press(MousePress::Left) {
                    context.set_cursor(Some(MouseCursor::Hand));
                    self.split_drag_start.replace(split);
                    return;
                }
            }
        }

        for pos in self.get_panes_to_render() {
            if y >= pos.top as i64
                && y <= (pos.top + pos.height) as i64
                && x >= pos.left
                && x <= pos.left + pos.width
            {
                if pane.pane_id() != pos.pane.pane_id() {
                    // We're over a pane that isn't active
                    match &event.kind {
                        WMEK::Press(_) => {
                            let mux = Mux::get().unwrap();
                            mux.get_active_tab_for_window(self.mux_window_id)
                                .map(|tab| tab.set_active_idx(pos.index));

                            pane = Rc::clone(&pos.pane);
                        }
                        WMEK::Move => {}
                        WMEK::Release(_) => {}
                        WMEK::VertWheel(_) => {}
                        WMEK::HorzWheel(_) => {}
                    }
                }
                x = x.saturating_sub(pos.left);
                y = y.saturating_sub(pos.top as i64);
                break;
            }
        }

        let dims = pane.renderer().get_dimensions();
        let stable_row = self
            .get_viewport(pane.pane_id())
            .unwrap_or(dims.physical_top)
            + y as StableRowIndex;

        self.last_mouse_terminal_coords = (x, stable_row); // FIXME: per-pane

        let (top, mut lines) = pane.renderer().get_lines(stable_row..stable_row + 1);
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

        context.set_cursor(Some(if on_split || self.current_highlight.is_some() {
            // When hovering over a hyperlink, show an appropriate
            // mouse cursor to give the cue that it is clickable
            MouseCursor::Hand
        } else {
            MouseCursor::Text
        }));

        let event_trigger_type = match &event.kind {
            WMEK::Press(press) => {
                let press = mouse_press_to_tmb(press);
                match self.last_mouse_click.as_ref() {
                    Some(LastMouseClick { streak, button, .. }) if *button == press => {
                        Some(MouseEventTrigger::Down {
                            streak: *streak,
                            button: press,
                        })
                    }
                    _ => None,
                }
            }
            WMEK::Release(press) => {
                let press = mouse_press_to_tmb(press);
                match self.last_mouse_click.as_ref() {
                    Some(LastMouseClick { streak, button, .. }) if *button == press => {
                        Some(MouseEventTrigger::Up {
                            streak: *streak,
                            button: press,
                        })
                    }
                    _ => None,
                }
            }
            WMEK::Move => {
                if let Some(LastMouseClick { streak, button, .. }) = self.last_mouse_click.as_ref()
                {
                    if Some(*button) == self.current_mouse_button.as_ref().map(mouse_press_to_tmb) {
                        Some(MouseEventTrigger::Drag {
                            streak: *streak,
                            button: *button,
                        })
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            WMEK::VertWheel(_) | WMEK::HorzWheel(_) => None,
        };

        let ignore_grab_modifier = Modifiers::SHIFT;

        if !pane.is_mouse_grabbed() || event.modifiers.contains(ignore_grab_modifier) {
            let event_trigger_type = match event_trigger_type {
                Some(ett) => ett,
                None => return,
            };

            let mut modifiers = window_mods_to_termwiz_mods(event.modifiers);

            // Since we use shift to force assessing the mouse bindings, pretend
            // that shift is not one of the mods when the mouse is grabbed.
            if pane.is_mouse_grabbed() {
                modifiers -= window_mods_to_termwiz_mods(ignore_grab_modifier);
            }

            if let Some(action) = self
                .input_map
                .lookup_mouse(event_trigger_type.clone(), modifiers)
            {
                self.perform_key_assignment(&pane, &action).ok();
            }

            return;
        }

        let mouse_event = wezterm_term::MouseEvent {
            kind: match event.kind {
                WMEK::Move => TMEK::Move,
                WMEK::VertWheel(_) | WMEK::HorzWheel(_) | WMEK::Press(_) => TMEK::Press,
                WMEK::Release(_) => TMEK::Release,
            },
            button: match event.kind {
                WMEK::Release(ref press) | WMEK::Press(ref press) => mouse_press_to_tmb(press),
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

        pane.mouse_event(mouse_event).ok();

        match event.kind {
            WMEK::Move => {}
            _ => {
                context.invalidate();
            }
        }
    }

    fn maybe_scroll_to_bottom_for_input(&mut self, pane: &Rc<dyn Pane>) {
        if configuration().scroll_to_bottom_on_input {
            self.scroll_to_bottom(pane);
        }
    }

    fn scroll_to_bottom(&mut self, pane: &Rc<dyn Pane>) {
        self.pane_state(pane.pane_id()).viewport = None;
    }

    fn get_active_pane_no_overlay(&self) -> Option<Rc<dyn Pane>> {
        let mux = Mux::get().unwrap();
        mux.get_active_tab_for_window(self.mux_window_id)
            .and_then(|tab| tab.get_active_pane())
    }

    /// Returns a Pane that we can interact with; this will typically be
    /// the active tab for the window, but if the window has a tab-wide
    /// overlay (such as the launcher / tab navigator),
    /// then that will be returned instead.  Otherwise, if the pane has
    /// an active overlay (such as search or copy mode) then that will
    /// be returned.
    fn get_active_pane_or_overlay(&self) -> Option<Rc<dyn Pane>> {
        let mux = Mux::get().unwrap();
        let tab = match mux.get_active_tab_for_window(self.mux_window_id) {
            Some(tab) => tab,
            None => return None,
        };

        let tab_id = tab.tab_id();

        if let Some(tab_overlay) = self.tab_state(tab_id).overlay.clone() {
            Some(tab_overlay)
        } else {
            let pane = tab.get_active_pane()?;
            let pane_id = pane.pane_id();
            self.pane_state(pane_id)
                .overlay
                .clone()
                .or_else(|| Some(pane))
        }
    }

    fn get_splits(&mut self) -> Vec<PositionedSplit> {
        let mux = Mux::get().unwrap();
        let tab = match mux.get_active_tab_for_window(self.mux_window_id) {
            Some(tab) => tab,
            None => return vec![],
        };

        let tab_id = tab.tab_id();

        if let Some(_) = self.tab_state(tab_id).overlay.clone() {
            vec![]
        } else {
            tab.iter_splits()
        }
    }

    fn get_panes_to_render(&mut self) -> Vec<PositionedPane> {
        let mux = Mux::get().unwrap();
        let tab = match mux.get_active_tab_for_window(self.mux_window_id) {
            Some(tab) => tab,
            None => return vec![],
        };

        let tab_id = tab.tab_id();

        if let Some(pane) = self.tab_state(tab_id).overlay.clone() {
            let size = tab.get_size();
            vec![PositionedPane {
                index: 0,
                is_active: true,
                left: 0,
                top: 0,
                width: size.cols as _,
                height: size.rows as _,
                pane,
            }]
        } else {
            let mut panes = tab.iter_panes();
            for p in &mut panes {
                if let Some(overlay) = self.pane_state(p.pane.pane_id()).overlay.as_ref() {
                    p.pane = Rc::clone(overlay);
                }
            }
            panes
        }
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

    fn cancel_overlay_for_pane(&self, pane_id: PaneId) {
        self.pane_state(pane_id).overlay.take();
        if let Some(window) = self.window.as_ref() {
            window.invalidate();
        }
    }

    pub fn schedule_cancel_overlay_for_pane(window: Window, pane_id: PaneId) {
        window.apply(move |myself, _| {
            if let Some(myself) = myself.downcast_mut::<Self>() {
                myself.cancel_overlay_for_pane(pane_id);
            }
            Ok(())
        });
    }

    pub fn assign_overlay_for_pane(&mut self, pane_id: PaneId, overlay: Rc<dyn Pane>) {
        self.pane_state(pane_id).overlay.replace(overlay);
        self.update_title();
    }

    pub fn assign_overlay(&mut self, tab_id: TabId, overlay: Rc<dyn Pane>) {
        self.tab_state(tab_id).overlay.replace(overlay);
        self.update_title();
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
    if modifiers.contains(::window::Modifiers::LEFT_ALT) {
        result.insert(termwiz::input::Modifiers::ALT);
    }
    if modifiers.contains(::window::Modifiers::RIGHT_ALT) {
        result.insert(termwiz::input::Modifiers::ALT);
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
