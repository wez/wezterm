//! This file is derived from the ConceptFrame implementation
//! in smithay_client_toolkit 0.11 which is Copyright (c) 2018 Victor Berger
//! and provided under the terms of the MIT license.

use smithay_client_toolkit::output::{add_output_listener, with_output_info, OutputListener};
use smithay_client_toolkit::seat::pointer::{ThemeManager, ThemeSpec, ThemedPointer};
use smithay_client_toolkit::shm::DoubleMemPool;
use smithay_client_toolkit::window::{ButtonState, Frame, FrameRequest, State, WindowState};
use std::cell::RefCell;
use std::cmp::max;
use std::rc::Rc;
use std::sync::Mutex;
use tiny_skia::{FillRule, Paint, PathBuilder, PixmapMut, Rect, Stroke, Transform};
use wayland_client::protocol::{
    wl_compositor, wl_output, wl_pointer, wl_seat, wl_shm, wl_subcompositor, wl_subsurface,
    wl_surface,
};
use wayland_client::{Attached, DispatchData, Main};
use wezterm_font::FontConfiguration;

/// Unambiguous representation of an ARGB color
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct ARGBColor {
    /// Alpha channel
    pub a: u8,
    /// Red channel
    pub r: u8,
    /// Green channel
    pub g: u8,
    /// Blue channel
    pub b: u8,
}

impl ARGBColor {
    /// The invisible `#00000000` color
    pub const fn zero() -> ARGBColor {
        ARGBColor {
            a: 0,
            r: 0,
            g: 0,
            b: 0,
        }
    }

    pub fn paint(&self) -> Paint {
        let mut paint = Paint::default();
        paint.set_color_rgba8(self.b, self.g, self.r, self.a);
        paint.anti_alias = true;
        paint
    }
}

impl From<ARGBColor> for [u8; 4] {
    fn from(color: ARGBColor) -> [u8; 4] {
        [color.a, color.r, color.g, color.b]
    }
}

impl From<[u8; 4]> for ARGBColor {
    fn from(array: [u8; 4]) -> ARGBColor {
        ARGBColor {
            a: array[0],
            r: array[1],
            g: array[2],
            b: array[3],
        }
    }
}

/// Color specification to be used in Frame configuration
///
/// It regroups two colors, one for when the window is active and
/// one for when it is not.
#[derive(Copy, Clone, Debug)]
pub struct ColorSpec {
    /// The active color
    pub active: ARGBColor,
    /// The inactive color
    pub inactive: ARGBColor,
}

impl ColorSpec {
    /// Access the color associated with a certain window state
    #[inline]
    pub fn get_for(self, state: WindowState) -> ARGBColor {
        match state {
            WindowState::Active => self.active,
            WindowState::Inactive => self.inactive,
        }
    }

    /// Create a ColorSpec that is always the same color
    #[inline]
    pub const fn identical(color: ARGBColor) -> ColorSpec {
        ColorSpec {
            active: color,
            inactive: color,
        }
    }

    /// Create a ColorSpec corresponding to an always invisible color
    #[inline]
    pub const fn invisible() -> ColorSpec {
        ColorSpec::identical(ARGBColor::zero())
    }
}

/// A color specification associated with a button
///
/// It regroups 3 color specifications depending on the state of the
/// button: idle, hovered, or disabled.
#[derive(Copy, Clone, Debug)]
pub struct ButtonColorSpec {
    /// ColorSpec for an idle button
    pub idle: ColorSpec,
    /// ColorSpec for an hovered button
    pub hovered: ColorSpec,
    /// ColorSpec for a disabled button
    pub disabled: ColorSpec,
}

impl ButtonColorSpec {
    /// Get the ColorSpec associated with a given button state
    pub fn get_for(&self, state: ButtonState) -> ColorSpec {
        match state {
            ButtonState::Idle => self.idle,
            ButtonState::Hovered => self.hovered,
            ButtonState::Disabled => self.disabled,
        }
    }
}

/*
 * Drawing theme definitions
 */

const BORDER_SIZE: u32 = 12;
const HEADER_SIZE: u32 = 30;

/// Configuration for ConceptFrame
#[derive(Clone)]
pub struct ConceptConfig {
    /// The primary color of the titlebar
    pub primary_color: ColorSpec,
    /// Secondary color of the theme
    ///
    /// Used for the division line between the titlebar and the content
    pub secondary_color: ColorSpec,
    /// Parameters of the "Close" (or "x") button
    ///
    /// (icon color, button color)
    ///
    /// if `None` the button will not be drawn
    pub close_button: Option<(ButtonColorSpec, ButtonColorSpec)>,
    /// Parameters of the "Maximize" (or "^") button
    ///
    /// (icon color, button color)
    ///
    /// if `None` the button will not be drawn
    pub maximize_button: Option<(ButtonColorSpec, ButtonColorSpec)>,
    /// Parameters of the "Minimize" (or "v") button
    ///
    /// (icon color, button color)
    ///
    /// if `None` the button will not be drawn
    pub minimize_button: Option<(ButtonColorSpec, ButtonColorSpec)>,
    /// Font configuration for the titlebar
    ///
    /// Font name and size. If set to `None`, the title is not drawn.
    pub title_font: Option<(String, f32)>,
    /// Color for drawing the title text
    pub title_color: ColorSpec,

    pub font_config: Option<Rc<FontConfiguration>>,
}

impl Default for ConceptConfig {
    fn default() -> ConceptConfig {
        let icon_spec = ButtonColorSpec {
            idle: ColorSpec::identical([0xFF, 0x1E, 0x1E, 0x1E].into()),
            hovered: ColorSpec::identical([0xFF, 0x1E, 0x1E, 0x1E].into()),
            disabled: ColorSpec::invisible(),
        };

        ConceptConfig {
            font_config: None,
            primary_color: ColorSpec {
                active: [0xFF, 0xE6, 0xE6, 0xE6].into(),
                inactive: [0xFF, 0xDC, 0xDC, 0xDC].into(),
            },
            secondary_color: ColorSpec {
                active: [0xFF, 0x1E, 0x1E, 0x1E].into(),
                inactive: [0xFF, 0x78, 0x78, 0x78].into(),
            },
            close_button: Some((
                // icon
                icon_spec,
                // button background
                ButtonColorSpec {
                    idle: ColorSpec::invisible(),
                    hovered: ColorSpec::identical([0xFF, 0xD9, 0x43, 0x52].into()),
                    disabled: ColorSpec::invisible(),
                },
            )),
            maximize_button: Some((
                // icon
                icon_spec,
                // button background
                ButtonColorSpec {
                    idle: ColorSpec::invisible(),
                    hovered: ColorSpec::identical([0xFF, 0x2D, 0xCB, 0x70].into()),
                    disabled: ColorSpec::invisible(),
                },
            )),
            minimize_button: Some((
                // icon
                icon_spec,
                // button background
                ButtonColorSpec {
                    idle: ColorSpec::invisible(),
                    hovered: ColorSpec::identical([0xFF, 0x3C, 0xAD, 0xE8].into()),
                    disabled: ColorSpec::invisible(),
                },
            )),
            title_font: Some(("sans".into(), 17.0)),
            title_color: ColorSpec::identical([0xFF, 0x00, 0x00, 0x00].into()),
        }
    }
}

/*
 * Utilities
 */

const HEAD: usize = 0;
const TOP: usize = 1;
const BOTTOM: usize = 2;
const LEFT: usize = 3;
const RIGHT: usize = 4;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum Location {
    None,
    Head,
    Top,
    TopRight,
    Right,
    BottomRight,
    Bottom,
    BottomLeft,
    Left,
    TopLeft,
    Button(UIButton),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum UIButton {
    Minimize,
    Maximize,
    Close,
}

struct Part {
    surface: wl_surface::WlSurface,
    subsurface: wl_subsurface::WlSubsurface,
}

pub(crate) struct SurfaceUserData {
    scale_factor: i32,
    outputs: Vec<(wl_output::WlOutput, i32, OutputListener)>,
}

impl SurfaceUserData {
    fn new() -> Self {
        SurfaceUserData {
            scale_factor: 1,
            outputs: Vec::new(),
        }
    }

    pub(crate) fn enter<F>(
        &mut self,
        output: wl_output::WlOutput,
        surface: wl_surface::WlSurface,
        callback: &Option<Rc<RefCell<F>>>,
    ) where
        F: FnMut(i32, wl_surface::WlSurface, DispatchData) + 'static,
    {
        let output_scale = with_output_info(&output, |info| info.scale_factor).unwrap_or(1);
        let my_surface = surface.clone();
        // Use a UserData to safely share the callback with the other thread
        let my_callback = wayland_client::UserData::new();
        if let Some(ref cb) = callback {
            my_callback.set(|| cb.clone());
        }
        let listener = add_output_listener(&output, move |output, info, ddata| {
            let mut user_data = my_surface
                .as_ref()
                .user_data()
                .get::<Mutex<SurfaceUserData>>()
                .unwrap()
                .lock()
                .unwrap();
            // update the scale factor of the relevant output
            for (ref o, ref mut factor, _) in user_data.outputs.iter_mut() {
                if o.as_ref().equals(output.as_ref()) {
                    if info.obsolete {
                        // an output that no longer exists is marked by a scale factor of -1
                        *factor = -1;
                    } else {
                        *factor = info.scale_factor;
                    }
                    break;
                }
            }
            // recompute the scale factor with the new info
            let callback = my_callback.get::<Rc<RefCell<F>>>().cloned();
            let old_scale_factor = user_data.scale_factor;
            let new_scale_factor = user_data.recompute_scale_factor();
            drop(user_data);
            if let Some(ref cb) = callback {
                if old_scale_factor != new_scale_factor {
                    (&mut *cb.borrow_mut())(new_scale_factor, surface.clone(), ddata);
                }
            }
        });
        self.outputs.push((output, output_scale, listener));
    }

    pub(crate) fn leave(&mut self, output: &wl_output::WlOutput) {
        self.outputs
            .retain(|(ref output2, _, _)| !output.as_ref().equals(output2.as_ref()));
    }

    fn recompute_scale_factor(&mut self) -> i32 {
        let mut new_scale_factor = 1;
        self.outputs.retain(|&(_, output_scale, _)| {
            if output_scale > 0 {
                new_scale_factor = ::std::cmp::max(new_scale_factor, output_scale);
                true
            } else {
                // cleanup obsolete output
                false
            }
        });
        if self.outputs.is_empty() {
            // don't update the scale factor if we are not displayed on any output
            return self.scale_factor;
        }
        self.scale_factor = new_scale_factor;
        new_scale_factor
    }
}

/// Returns the current suggested scale factor of a surface.
///
/// Panics if the surface was not created using `create_surface`
fn get_surface_scale_factor(surface: &wl_surface::WlSurface) -> i32 {
    surface
        .as_ref()
        .user_data()
        .get::<Mutex<SurfaceUserData>>()
        .expect("SCTK: Surface was not created by SCTK.")
        .lock()
        .unwrap()
        .scale_factor
}

fn setup_surface<F>(
    surface: Main<wl_surface::WlSurface>,
    callback: Option<F>,
) -> Attached<wl_surface::WlSurface>
where
    F: FnMut(i32, wl_surface::WlSurface, DispatchData) + 'static,
{
    let callback = callback.map(|c| Rc::new(RefCell::new(c)));
    surface.quick_assign(move |surface, event, ddata| {
        let mut user_data = surface
            .as_ref()
            .user_data()
            .get::<Mutex<SurfaceUserData>>()
            .unwrap()
            .lock()
            .unwrap();
        match event {
            wl_surface::Event::Enter { output } => {
                // Passing the callback to be added to output listener
                user_data.enter(output, surface.detach(), &callback);
            }
            wl_surface::Event::Leave { output } => {
                user_data.leave(&output);
            }
            _ => unreachable!(),
        };
        let old_scale_factor = user_data.scale_factor;
        let new_scale_factor = user_data.recompute_scale_factor();
        drop(user_data);
        if let Some(ref cb) = callback {
            if old_scale_factor != new_scale_factor {
                (&mut *cb.borrow_mut())(new_scale_factor, surface.detach(), ddata);
            }
        }
    });
    surface
        .as_ref()
        .user_data()
        .set_threadsafe(|| Mutex::new(SurfaceUserData::new()));
    surface.into()
}

impl Part {
    fn new(
        parent: &wl_surface::WlSurface,
        compositor: &Attached<wl_compositor::WlCompositor>,
        subcompositor: &Attached<wl_subcompositor::WlSubcompositor>,
        inner: Option<Rc<RefCell<Inner>>>,
    ) -> Part {
        let surface = if let Some(inner) = inner {
            setup_surface(
                compositor.create_surface(),
                Some(
                    move |dpi, surface: wl_surface::WlSurface, ddata: DispatchData| {
                        surface.set_buffer_scale(dpi);
                        surface.commit();
                        (&mut inner.borrow_mut().implem)(FrameRequest::Refresh, 0, ddata);
                    },
                ),
            )
        } else {
            setup_surface(
                compositor.create_surface(),
                Some(
                    move |dpi, surface: wl_surface::WlSurface, _ddata: DispatchData| {
                        surface.set_buffer_scale(dpi);
                        surface.commit();
                    },
                ),
            )
        };

        let surface = surface.detach();

        let subsurface = subcompositor.get_subsurface(&surface, parent);

        Part {
            surface,
            subsurface: subsurface.detach(),
        }
    }
}

impl Drop for Part {
    fn drop(&mut self) {
        self.subsurface.destroy();
        self.surface.destroy();
    }
}

struct PointerUserData {
    location: Location,
    position: (f64, f64),
    seat: wl_seat::WlSeat,
}

/*
 * The core frame
 */

struct Inner {
    parts: Vec<Part>,
    size: (u32, u32),
    resizable: bool,
    theme_over_surface: bool,
    implem: Box<dyn FnMut(FrameRequest, u32, DispatchData)>,
    maximized: bool,
    fullscreened: bool,
    buttons: (bool, bool, bool),
}

impl Inner {
    fn find_surface(&self, surface: &wl_surface::WlSurface) -> Location {
        if self.parts.is_empty() {
            return Location::None;
        }

        if surface.as_ref().equals(self.parts[HEAD].surface.as_ref()) {
            Location::Head
        } else if surface.as_ref().equals(self.parts[TOP].surface.as_ref()) {
            Location::Top
        } else if surface.as_ref().equals(self.parts[BOTTOM].surface.as_ref()) {
            Location::Bottom
        } else if surface.as_ref().equals(self.parts[LEFT].surface.as_ref()) {
            Location::Left
        } else if surface.as_ref().equals(self.parts[RIGHT].surface.as_ref()) {
            Location::Right
        } else {
            Location::None
        }
    }
}

fn precise_location(
    old: Location,
    width: u32,
    x: f64,
    y: f64,
    buttons: (bool, bool, bool),
) -> Location {
    match old {
        Location::Head | Location::Button(_) => find_button(x, y, width, buttons),

        Location::Top | Location::TopLeft | Location::TopRight => {
            if x <= f64::from(BORDER_SIZE) {
                Location::TopLeft
            } else if x >= f64::from(width + BORDER_SIZE) {
                Location::TopRight
            } else {
                Location::Top
            }
        }

        Location::Bottom | Location::BottomLeft | Location::BottomRight => {
            if x <= f64::from(BORDER_SIZE) {
                Location::BottomLeft
            } else if x >= f64::from(width + BORDER_SIZE) {
                Location::BottomRight
            } else {
                Location::Bottom
            }
        }

        other => other,
    }
}

fn find_button(x: f64, y: f64, w: u32, buttons: (bool, bool, bool)) -> Location {
    if (w >= HEADER_SIZE)
        && (x >= f64::from(w - HEADER_SIZE))
        && (x <= f64::from(w))
        && (y <= f64::from(HEADER_SIZE))
        && (y >= f64::from(0))
    {
        // first button
        match buttons {
            (true, _, _) => Location::Button(UIButton::Close),
            (false, true, _) => Location::Button(UIButton::Maximize),
            (false, false, true) => Location::Button(UIButton::Minimize),
            _ => Location::Head,
        }
    } else if (w >= 2 * HEADER_SIZE)
        && (x >= f64::from(w - 2 * HEADER_SIZE))
        && (x <= f64::from(w - HEADER_SIZE))
        && (y <= f64::from(HEADER_SIZE))
        && (y >= f64::from(0))
    {
        // second button
        match buttons {
            (true, true, _) => Location::Button(UIButton::Maximize),
            (false, true, true) => Location::Button(UIButton::Minimize),
            _ => Location::Head,
        }
    } else if (w >= 3 * HEADER_SIZE)
        && (x >= f64::from(w - 3 * HEADER_SIZE))
        && (x <= f64::from(w - 2 * HEADER_SIZE))
        && (y <= f64::from(HEADER_SIZE))
        && (y >= f64::from(0))
    {
        // third button
        match buttons {
            (true, true, true) => Location::Button(UIButton::Minimize),
            _ => Location::Head,
        }
    } else {
        Location::Head
    }
}

/// A clean, modern and stylish set of decorations.
///
/// This class draws clean and modern decorations with
/// buttons inspired by breeze, material hover shade and
/// a white header background.
///
/// `ConceptFrame` is hiding its `ClientSide` decorations
/// in a `Fullscreen` state and brings them back if those are
/// visible when unsetting `Fullscreen` state.
pub struct ConceptFrame {
    base_surface: wl_surface::WlSurface,
    compositor: Attached<wl_compositor::WlCompositor>,
    subcompositor: Attached<wl_subcompositor::WlSubcompositor>,
    inner: Rc<RefCell<Inner>>,
    pools: DoubleMemPool,
    active: WindowState,
    hidden: bool,
    pointers: Vec<ThemedPointer>,
    themer: ThemeManager,
    surface_version: u32,
    config: ConceptConfig,
    title: Option<String>,
}

impl Frame for ConceptFrame {
    type Error = ::std::io::Error;
    type Config = ConceptConfig;
    fn init(
        base_surface: &wl_surface::WlSurface,
        compositor: &Attached<wl_compositor::WlCompositor>,
        subcompositor: &Attached<wl_subcompositor::WlSubcompositor>,
        shm: &Attached<wl_shm::WlShm>,
        theme_manager: Option<ThemeManager>,
        implementation: Box<dyn FnMut(FrameRequest, u32, DispatchData)>,
    ) -> Result<ConceptFrame, ::std::io::Error> {
        let (themer, theme_over_surface) = if let Some(theme_manager) = theme_manager {
            (theme_manager, false)
        } else {
            (
                ThemeManager::init(ThemeSpec::System, compositor.clone(), shm.clone()),
                true,
            )
        };

        let inner = Rc::new(RefCell::new(Inner {
            parts: vec![],
            size: (1, 1),
            resizable: true,
            implem: implementation,
            theme_over_surface,
            maximized: false,
            fullscreened: false,
            buttons: (true, true, true),
        }));

        let my_inner = inner.clone();
        // Send a Refresh request on callback from DoubleMemPool as it will be fired when
        // None was previously returned from `pool()` and the draw was postponed
        let pools = DoubleMemPool::new(shm.clone(), move |ddata| {
            (&mut my_inner.borrow_mut().implem)(FrameRequest::Refresh, 0, ddata);
        })?;

        Ok(ConceptFrame {
            base_surface: base_surface.clone(),
            compositor: compositor.clone(),
            subcompositor: subcompositor.clone(),
            inner,
            pools,
            active: WindowState::Inactive,
            hidden: true,
            pointers: Vec::new(),
            themer,
            surface_version: compositor.as_ref().version(),
            config: ConceptConfig::default(),
            title: None,
        })
    }

    fn new_seat(&mut self, seat: &Attached<wl_seat::WlSeat>) {
        use self::wl_pointer::Event;
        let inner = self.inner.clone();
        let pointer = self.themer.theme_pointer_with_impl(
            seat,
            move |event, pointer: ThemedPointer, ddata: DispatchData| {
                let data: &RefCell<PointerUserData> = pointer.as_ref().user_data().get().unwrap();
                let mut data = data.borrow_mut();
                let mut inner = inner.borrow_mut();
                match event {
                    Event::Enter {
                        serial,
                        surface,
                        surface_x,
                        surface_y,
                    } => {
                        data.location = precise_location(
                            inner.find_surface(&surface),
                            inner.size.0,
                            surface_x,
                            surface_y,
                            inner.buttons,
                        );
                        data.position = (surface_x, surface_y);
                        change_pointer(&pointer, &inner, data.location, Some(serial))
                    }
                    Event::Leave { serial, .. } => {
                        data.location = Location::None;
                        change_pointer(&pointer, &inner, data.location, Some(serial));
                        (&mut inner.implem)(FrameRequest::Refresh, 0, ddata);
                    }
                    Event::Motion {
                        surface_x,
                        surface_y,
                        ..
                    } => {
                        data.position = (surface_x, surface_y);
                        let newpos = precise_location(
                            data.location,
                            inner.size.0,
                            surface_x,
                            surface_y,
                            inner.buttons,
                        );
                        if newpos != data.location {
                            match (newpos, data.location) {
                                (Location::Button(_), _) | (_, Location::Button(_)) => {
                                    // pointer movement involves a button, request refresh
                                    (&mut inner.implem)(FrameRequest::Refresh, 0, ddata);
                                }
                                _ => (),
                            }
                            // we changed of part of the decoration, pointer image
                            // may need to be changed
                            data.location = newpos;
                            change_pointer(&pointer, &inner, data.location, None)
                        }
                    }
                    Event::Button {
                        serial,
                        button,
                        state,
                        ..
                    } => {
                        if state == wl_pointer::ButtonState::Pressed {
                            let request = match button {
                                // Left mouse button.
                                0x110 => request_for_location_on_lmb(
                                    &data,
                                    inner.maximized,
                                    inner.resizable,
                                ),
                                // Right mouse button.
                                0x111 => request_for_location_on_rmb(&data),
                                _ => None,
                            };

                            if let Some(request) = request {
                                (&mut inner.implem)(request, serial, ddata);
                            }
                        }
                    }
                    _ => {}
                }
            },
        );
        pointer.as_ref().user_data().set(|| {
            RefCell::new(PointerUserData {
                location: Location::None,
                position: (0.0, 0.0),
                seat: seat.detach(),
            })
        });
        self.pointers.push(pointer);
    }

    fn remove_seat(&mut self, seat: &wl_seat::WlSeat) {
        self.pointers.retain(|pointer| {
            let user_data = pointer
                .as_ref()
                .user_data()
                .get::<RefCell<PointerUserData>>()
                .unwrap();
            let guard = user_data.borrow_mut();
            if &guard.seat == seat {
                pointer.release();
                false
            } else {
                true
            }
        });
    }

    fn set_states(&mut self, states: &[State]) -> bool {
        let mut inner = self.inner.borrow_mut();
        let mut need_redraw = false;

        // Process active.
        let new_active = if states.contains(&State::Activated) {
            WindowState::Active
        } else {
            WindowState::Inactive
        };
        need_redraw |= new_active != self.active;
        self.active = new_active;

        // Process maximized.
        let new_maximized = states.contains(&State::Maximized);
        need_redraw |= new_maximized != inner.maximized;
        inner.maximized = new_maximized;

        // Process fullscreened.
        let new_fullscreened = states.contains(&State::Fullscreen);
        need_redraw |= new_fullscreened != inner.fullscreened;
        inner.fullscreened = new_fullscreened;

        need_redraw
    }

    fn set_hidden(&mut self, hidden: bool) {
        self.hidden = hidden;
        let mut inner = self.inner.borrow_mut();
        if !self.hidden {
            if inner.parts.is_empty() {
                inner.parts = vec![
                    Part::new(
                        &self.base_surface,
                        &self.compositor,
                        &self.subcompositor,
                        Some(Rc::clone(&self.inner)),
                    ),
                    Part::new(
                        &self.base_surface,
                        &self.compositor,
                        &self.subcompositor,
                        None,
                    ),
                    Part::new(
                        &self.base_surface,
                        &self.compositor,
                        &self.subcompositor,
                        None,
                    ),
                    Part::new(
                        &self.base_surface,
                        &self.compositor,
                        &self.subcompositor,
                        None,
                    ),
                    Part::new(
                        &self.base_surface,
                        &self.compositor,
                        &self.subcompositor,
                        None,
                    ),
                ];
            }
        } else {
            inner.parts.clear();
        }
    }

    fn set_resizable(&mut self, resizable: bool) {
        self.inner.borrow_mut().resizable = resizable;
    }

    fn resize(&mut self, newsize: (u32, u32)) {
        self.inner.borrow_mut().size = newsize;
    }

    fn redraw(&mut self) {
        let inner = self.inner.borrow_mut();

        // Don't draw borders if the frame explicitly hidden or fullscreened.
        if self.hidden || inner.fullscreened {
            // Don't draw the borders.
            for p in inner.parts.iter() {
                p.surface.attach(None, 0, 0);
                p.surface.commit();
            }
            return;
        }

        // `parts` can't be empty here, since the initial state for `self.hidden` is true, and
        // they will be created once `self.hidden` will become `false`.
        let parts = &inner.parts;

        let scales: Vec<u32> = parts
            .iter()
            .map(|part| get_surface_scale_factor(&part.surface) as u32)
            .collect();

        let (width, height) = inner.size;

        // Use header scale for all the thing.
        let header_scale = scales[HEAD];

        let scaled_header_height = HEADER_SIZE * header_scale;
        let scaled_header_width = width * header_scale;

        {
            // grab the current pool
            let pool = match self.pools.pool() {
                Some(pool) => pool,
                None => return,
            };
            let lr_surfaces_scale = max(scales[LEFT], scales[RIGHT]);
            let tp_surfaces_scale = max(scales[TOP], scales[BOTTOM]);

            // resize the pool as appropriate
            let pxcount = (scaled_header_height * scaled_header_width)
                + max(
                    (width + 2 * BORDER_SIZE) * BORDER_SIZE * tp_surfaces_scale * tp_surfaces_scale,
                    (height + HEADER_SIZE) * BORDER_SIZE * lr_surfaces_scale * lr_surfaces_scale,
                );

            pool.resize(4 * pxcount as usize)
                .expect("I/O Error while redrawing the borders");

            // draw the white header bar
            {
                let mmap = pool.mmap();
                {
                    let color = self.config.primary_color.get_for(self.active);

                    let mut pixmap = PixmapMut::from_bytes(
                        &mut mmap
                            [0..scaled_header_height as usize * scaled_header_width as usize * 4],
                        scaled_header_width,
                        scaled_header_height,
                    )
                    .expect("make pixmap from existing bitmap");

                    pixmap.fill_path(
                        &PathBuilder::from_rect(
                            Rect::from_xywh(
                                0.,
                                0.,
                                scaled_header_width as f32,
                                scaled_header_height as f32,
                            )
                            .unwrap(),
                        ),
                        &color.paint(),
                        FillRule::Winding,
                        Transform::identity(),
                        None,
                    );

                    draw_buttons(
                        &mut pixmap,
                        width,
                        header_scale,
                        inner.resizable,
                        self.active,
                        &self
                            .pointers
                            .iter()
                            .flat_map(|p| {
                                if p.as_ref().is_alive() {
                                    let data: &RefCell<PointerUserData> =
                                        p.as_ref().user_data().get().unwrap();
                                    Some(data.borrow().location)
                                } else {
                                    None
                                }
                            })
                            .collect::<Vec<Location>>(),
                        &self.config,
                    );
                    /*
                    if let Some((ref font_face, font_size)) = self.config.title_font {
                        if let Some(title) = self.title.clone() {
                            // If theres no stored font data, find the first ttf regular sans font and
                            // store it
                            if self.font_data.is_none() {
                                if let Some(font) = fontconfig::FontConfig::new()
                                    .unwrap()
                                    .get_regular_family_fonts(&font_face)
                                    .unwrap()
                                    .iter()
                                    .find(|p| p.extension().map(|e| e == "ttf").unwrap_or(false))
                                {
                                    let mut font_data = Vec::new();
                                    if let Ok(mut file) = ::std::fs::File::open(font) {
                                        match file.read_to_end(&mut font_data) {
                                            Ok(_) => self.font_data = Some(font_data),
                                            Err(err) => {
                                                log::error!("Could not read font file: {}", err)
                                            }
                                        }
                                    }
                                }
                            }

                            // Create text from stored title and font data
                            if let Some(ref font_data) = self.font_data {
                                let title_color = self.config.title_color.get_for(self.active);
                                let mut title_text = text::Text::new(
                                    (
                                        0,
                                        (HEADER_SIZE as usize / 2)
                                            .saturating_sub((font_size / 2.0).ceil() as usize)
                                            * header_scale as usize,
                                    ),
                                    title_color.into(),
                                    font_data,
                                    font_size * header_scale as f32,
                                    1.0,
                                    title,
                                );

                                let mut button_count = 0isize;
                                if self.config.close_button.is_some() {
                                    button_count += 1;
                                }
                                if self.config.maximize_button.is_some() {
                                    button_count += 1;
                                }
                                if self.config.minimize_button.is_some() {
                                    button_count += 1;
                                }

                                let scaled_button_size =
                                    HEADER_SIZE as isize * header_scale as isize;
                                let button_space = button_count * scaled_button_size;
                                let scaled_header_width = width as isize * header_scale as isize;

                                // Check if text is bigger then the available width
                                if (scaled_header_width - button_space)
                                    > (title_text.get_width() as isize + scaled_button_size)
                                {
                                    title_text.pos.0 =
                                        (scaled_header_width - button_space) as usize / 2
                                            - (title_text.get_width() / 2);
                                    header_canvas.draw(&title_text);
                                }
                            }
                        }
                    }
                    */
                }

                // For each pixel in borders
                {
                    for b in &mut mmap
                        [scaled_header_height as usize * scaled_header_width as usize * 4..]
                    {
                        *b = 0x00;
                    }
                }
                if let Err(err) = mmap.flush() {
                    log::error!("Failed to flush frame memory map: {}", err);
                }
            }

            // Create the buffers
            // -> head-subsurface
            let buffer = pool.buffer(
                0,
                scaled_header_width as i32,
                scaled_header_height as i32,
                4 * scaled_header_width as i32,
                wl_shm::Format::Argb8888,
            );
            parts[HEAD]
                .subsurface
                .set_position(0, -(HEADER_SIZE as i32));
            parts[HEAD].surface.attach(Some(&buffer), 0, 0);
            if self.surface_version >= 4 {
                parts[HEAD].surface.damage_buffer(
                    0,
                    0,
                    scaled_header_width as i32,
                    scaled_header_height as i32,
                );
            } else {
                // surface is old and does not support damage_buffer, so we damage
                // in surface coordinates and hope it is not rescaled
                parts[HEAD]
                    .surface
                    .damage(0, 0, width as i32, HEADER_SIZE as i32);
            }
            parts[HEAD].surface.commit();

            // -> top-subsurface
            let buffer = pool.buffer(
                4 * (scaled_header_width * scaled_header_height) as i32,
                ((width + 2 * BORDER_SIZE) * scales[TOP]) as i32,
                (BORDER_SIZE * scales[TOP]) as i32,
                (4 * scales[TOP] * (width + 2 * BORDER_SIZE)) as i32,
                wl_shm::Format::Argb8888,
            );
            parts[TOP].subsurface.set_position(
                -(BORDER_SIZE as i32),
                -(HEADER_SIZE as i32 + BORDER_SIZE as i32),
            );
            parts[TOP].surface.attach(Some(&buffer), 0, 0);
            if self.surface_version >= 4 {
                parts[TOP].surface.damage_buffer(
                    0,
                    0,
                    ((width + 2 * BORDER_SIZE) * scales[TOP]) as i32,
                    (BORDER_SIZE * scales[TOP]) as i32,
                );
            } else {
                // surface is old and does not support damage_buffer, so we damage
                // in surface coordinates and hope it is not rescaled
                parts[TOP].surface.damage(
                    0,
                    0,
                    (width + 2 * BORDER_SIZE) as i32,
                    BORDER_SIZE as i32,
                );
            }
            parts[TOP].surface.commit();

            // -> bottom-subsurface
            let buffer = pool.buffer(
                4 * (scaled_header_width * scaled_header_height) as i32,
                ((width + 2 * BORDER_SIZE) * scales[BOTTOM]) as i32,
                (BORDER_SIZE * scales[BOTTOM]) as i32,
                (4 * scales[BOTTOM] * (width + 2 * BORDER_SIZE)) as i32,
                wl_shm::Format::Argb8888,
            );
            parts[BOTTOM]
                .subsurface
                .set_position(-(BORDER_SIZE as i32), height as i32);
            parts[BOTTOM].surface.attach(Some(&buffer), 0, 0);
            if self.surface_version >= 4 {
                parts[BOTTOM].surface.damage_buffer(
                    0,
                    0,
                    ((width + 2 * BORDER_SIZE) * scales[BOTTOM]) as i32,
                    (BORDER_SIZE * scales[BOTTOM]) as i32,
                );
            } else {
                // surface is old and does not support damage_buffer, so we damage
                // in surface coordinates and hope it is not rescaled
                parts[BOTTOM].surface.damage(
                    0,
                    0,
                    (width + 2 * BORDER_SIZE) as i32,
                    BORDER_SIZE as i32,
                );
            }
            parts[BOTTOM].surface.commit();

            // -> left-subsurface
            let buffer = pool.buffer(
                4 * (scaled_header_width * scaled_header_height) as i32,
                (BORDER_SIZE * scales[LEFT]) as i32,
                ((height + HEADER_SIZE) * scales[LEFT]) as i32,
                4 * (BORDER_SIZE * scales[LEFT]) as i32,
                wl_shm::Format::Argb8888,
            );
            parts[LEFT]
                .subsurface
                .set_position(-(BORDER_SIZE as i32), -(HEADER_SIZE as i32));
            parts[LEFT].surface.attach(Some(&buffer), 0, 0);
            if self.surface_version >= 4 {
                parts[LEFT].surface.damage_buffer(
                    0,
                    0,
                    (BORDER_SIZE * scales[LEFT]) as i32,
                    ((height + HEADER_SIZE) * scales[LEFT]) as i32,
                );
            } else {
                // surface is old and does not support damage_buffer, so we damage
                // in surface coordinates and hope it is not rescaled
                parts[LEFT]
                    .surface
                    .damage(0, 0, BORDER_SIZE as i32, (height + HEADER_SIZE) as i32);
            }
            parts[LEFT].surface.commit();

            // -> right-subsurface
            let buffer = pool.buffer(
                4 * (scaled_header_width * scaled_header_height) as i32,
                (BORDER_SIZE * scales[RIGHT]) as i32,
                ((height + HEADER_SIZE) * scales[RIGHT]) as i32,
                4 * (BORDER_SIZE * scales[RIGHT]) as i32,
                wl_shm::Format::Argb8888,
            );
            parts[RIGHT]
                .subsurface
                .set_position(width as i32, -(HEADER_SIZE as i32));
            parts[RIGHT].surface.attach(Some(&buffer), 0, 0);
            if self.surface_version >= 4 {
                parts[RIGHT].surface.damage_buffer(
                    0,
                    0,
                    (BORDER_SIZE * scales[RIGHT]) as i32,
                    ((height + HEADER_SIZE) * scales[RIGHT]) as i32,
                );
            } else {
                // surface is old and does not support damage_buffer, so we damage
                // in surface coordinates and hope it is not rescaled
                parts[RIGHT].surface.damage(
                    0,
                    0,
                    BORDER_SIZE as i32,
                    (height + HEADER_SIZE) as i32,
                );
            }
            parts[RIGHT].surface.commit();
        }
    }

    fn subtract_borders(&self, width: i32, height: i32) -> (i32, i32) {
        if self.hidden || self.inner.borrow().fullscreened {
            (width, height)
        } else {
            (width, height - HEADER_SIZE as i32)
        }
    }

    fn add_borders(&self, width: i32, height: i32) -> (i32, i32) {
        if self.hidden || self.inner.borrow().fullscreened {
            (width, height)
        } else {
            (width, height + HEADER_SIZE as i32)
        }
    }

    fn location(&self) -> (i32, i32) {
        if self.hidden || self.inner.borrow().fullscreened {
            (0, 0)
        } else {
            (0, -(HEADER_SIZE as i32))
        }
    }

    fn set_config(&mut self, config: ConceptConfig) {
        self.config = config;
        let mut inner = self.inner.borrow_mut();
        inner.buttons = (
            self.config.close_button.is_some(),
            self.config.maximize_button.is_some(),
            self.config.minimize_button.is_some(),
        );
    }

    fn set_title(&mut self, title: String) {
        self.title = Some(title);
    }
}

impl Drop for ConceptFrame {
    fn drop(&mut self) {
        for ptr in self.pointers.drain(..) {
            if ptr.as_ref().version() >= 3 {
                ptr.release();
            }
        }
    }
}

fn change_pointer(pointer: &ThemedPointer, inner: &Inner, location: Location, serial: Option<u32>) {
    // Prevent theming of the surface if it was requested.
    if !inner.theme_over_surface && location == Location::None {
        return;
    }

    let name = match location {
        // If we can't resize a frame we shouldn't show resize cursors.
        _ if !inner.resizable => "left_ptr",
        Location::Top => "top_side",
        Location::TopRight => "top_right_corner",
        Location::Right => "right_side",
        Location::BottomRight => "bottom_right_corner",
        Location::Bottom => "bottom_side",
        Location::BottomLeft => "bottom_left_corner",
        Location::Left => "left_side",
        Location::TopLeft => "top_left_corner",
        _ => "left_ptr",
    };

    if pointer.set_cursor(name, serial).is_err() {
        log::error!("Failed to set cursor");
    }
}

fn request_for_location_on_lmb(
    pointer_data: &PointerUserData,
    maximized: bool,
    resizable: bool,
) -> Option<FrameRequest> {
    use wayland_protocols::xdg_shell::client::xdg_toplevel::ResizeEdge;
    match pointer_data.location {
        Location::Top if resizable => Some(FrameRequest::Resize(
            pointer_data.seat.clone(),
            ResizeEdge::Top,
        )),
        Location::TopLeft if resizable => Some(FrameRequest::Resize(
            pointer_data.seat.clone(),
            ResizeEdge::TopLeft,
        )),
        Location::Left if resizable => Some(FrameRequest::Resize(
            pointer_data.seat.clone(),
            ResizeEdge::Left,
        )),
        Location::BottomLeft if resizable => Some(FrameRequest::Resize(
            pointer_data.seat.clone(),
            ResizeEdge::BottomLeft,
        )),
        Location::Bottom if resizable => Some(FrameRequest::Resize(
            pointer_data.seat.clone(),
            ResizeEdge::Bottom,
        )),
        Location::BottomRight if resizable => Some(FrameRequest::Resize(
            pointer_data.seat.clone(),
            ResizeEdge::BottomRight,
        )),
        Location::Right if resizable => Some(FrameRequest::Resize(
            pointer_data.seat.clone(),
            ResizeEdge::Right,
        )),
        Location::TopRight if resizable => Some(FrameRequest::Resize(
            pointer_data.seat.clone(),
            ResizeEdge::TopRight,
        )),
        Location::Head => Some(FrameRequest::Move(pointer_data.seat.clone())),
        Location::Button(UIButton::Close) => Some(FrameRequest::Close),
        Location::Button(UIButton::Maximize) => {
            if maximized {
                Some(FrameRequest::UnMaximize)
            } else {
                Some(FrameRequest::Maximize)
            }
        }
        Location::Button(UIButton::Minimize) => Some(FrameRequest::Minimize),
        _ => None,
    }
}

fn request_for_location_on_rmb(pointer_data: &PointerUserData) -> Option<FrameRequest> {
    match pointer_data.location {
        Location::Head | Location::Button(_) => Some(FrameRequest::ShowMenu(
            pointer_data.seat.clone(),
            pointer_data.position.0 as i32,
            // We must offset it by header size for precise position.
            pointer_data.position.1 as i32 - HEADER_SIZE as i32,
        )),
        _ => None,
    }
}

// average of the two colors, approximately taking into account gamma correction
// result is as transparent as the most transparent color
fn mix_colors(x: ARGBColor, y: ARGBColor) -> ARGBColor {
    #[inline]
    fn gamma_mix(x: u8, y: u8) -> u8 {
        let x = x as f32 / 255.0;
        let y = y as f32 / 255.0;
        let z = ((x * x + y * y) / 2.0).sqrt();
        (z * 255.0) as u8
    }

    ARGBColor {
        a: x.a.min(y.a),
        r: gamma_mix(x.r, y.r),
        g: gamma_mix(x.g, y.g),
        b: gamma_mix(x.b, y.b),
    }
}

fn draw_buttons(
    pixmap: &mut PixmapMut,
    width: u32,
    scale: u32,
    maximizable: bool,
    state: WindowState,
    mouses: &[Location],
    config: &ConceptConfig,
) {
    let scale = scale as f32;

    // Draw seperator between header and window contents
    let line_color = config.secondary_color.get_for(state);

    let mut sep_stroke = Stroke::default();
    sep_stroke.width = scale;

    let mut path = PathBuilder::new();
    let y = HEADER_SIZE as f32 * scale - sep_stroke.width;
    path.move_to(0., y as f32);
    path.line_to(width as f32 * scale as f32, y);
    let path = path.finish().unwrap();

    pixmap.stroke_path(
        &path,
        &line_color.paint(),
        &Stroke::default(),
        Transform::identity(),
        None,
    );

    let mut drawn_buttons = 0usize;

    if width >= HEADER_SIZE {
        if let Some((ref icon_config, ref btn_config)) = config.close_button {
            // Draw the close button
            let btn_state = if mouses
                .iter()
                .any(|&l| l == Location::Button(UIButton::Close))
            {
                ButtonState::Hovered
            } else {
                ButtonState::Idle
            };

            let icon_color = icon_config.get_for(btn_state).get_for(state);
            let button_color = btn_config.get_for(btn_state).get_for(state);

            draw_button(
                pixmap,
                0,
                scale,
                button_color.paint(),
                mix_colors(button_color, line_color).paint(),
            );
            draw_icon(pixmap, 0, scale, icon_color.paint(), Icon::Close);
            drawn_buttons += 1;
        }
    }

    if width as usize >= (drawn_buttons + 1) * HEADER_SIZE as usize {
        if let Some((ref icon_config, ref btn_config)) = config.maximize_button {
            let btn_state = if !maximizable {
                ButtonState::Disabled
            } else if mouses
                .iter()
                .any(|&l| l == Location::Button(UIButton::Maximize))
            {
                ButtonState::Hovered
            } else {
                ButtonState::Idle
            };

            let icon_color = icon_config.get_for(btn_state).get_for(state);
            let button_color = btn_config.get_for(btn_state).get_for(state);

            draw_button(
                pixmap,
                drawn_buttons * HEADER_SIZE as usize,
                scale,
                button_color.paint(),
                mix_colors(button_color, line_color).paint(),
            );
            draw_icon(
                pixmap,
                drawn_buttons * HEADER_SIZE as usize,
                scale,
                icon_color.paint(),
                Icon::Maximize,
            );
            drawn_buttons += 1;
        }
    }

    if width as usize >= (drawn_buttons + 1) * HEADER_SIZE as usize {
        if let Some((ref icon_config, ref btn_config)) = config.minimize_button {
            let btn_state = if mouses
                .iter()
                .any(|&l| l == Location::Button(UIButton::Minimize))
            {
                ButtonState::Hovered
            } else {
                ButtonState::Idle
            };

            let icon_color = icon_config.get_for(btn_state).get_for(state);
            let button_color = btn_config.get_for(btn_state).get_for(state);

            draw_button(
                pixmap,
                drawn_buttons * HEADER_SIZE as usize,
                scale,
                button_color.paint(),
                mix_colors(button_color, line_color).paint(),
            );
            draw_icon(
                pixmap,
                drawn_buttons * HEADER_SIZE as usize,
                scale,
                icon_color.paint(),
                Icon::Minimize,
            );
        }
    }
}

enum Icon {
    Close,
    Maximize,
    Minimize,
}

fn draw_button(
    pixmap: &mut PixmapMut,
    x_offset: usize,
    scale: f32,
    btn_color: Paint,
    line_color: Paint,
) {
    let h = HEADER_SIZE as f32;
    let x_start = pixmap.width() as f32 / scale - h - x_offset as f32;
    // main square

    pixmap.fill_path(
        &PathBuilder::from_rect(
            Rect::from_xywh(x_start * scale, 0., h * scale, (h - 1.) * scale).unwrap(),
        ),
        &btn_color,
        FillRule::Winding,
        Transform::identity(),
        None,
    );

    // separation line

    let mut path = PathBuilder::new();
    path.move_to(x_start * scale, (h - 1.) * scale);
    path.line_to(x_start * scale, h * scale);
    let path = path.finish().unwrap();

    pixmap.stroke_path(
        &path,
        &line_color,
        &Stroke::default(),
        Transform::identity(),
        None,
    );
}

fn draw_icon(pixmap: &mut PixmapMut, x_offset: usize, scale: f32, icon_color: Paint, icon: Icon) {
    let h = HEADER_SIZE as f32;
    let cx = pixmap.width() as f32 / scale as f32 - h / 2. - x_offset as f32;
    let cy = h / 2.;
    let s = scale;

    let mut path = PathBuilder::new();
    let mut stroke = Stroke::default();
    stroke.width = 3.0;

    match icon {
        Icon::Close => {
            // Draw cross to represent the close button
            path.move_to((cx - 4.) * s, (cy - 4.) * s);
            path.line_to((cx + 4.) * s, (cy + 4.) * s);

            path.move_to((cx + 4.) * s, (cy - 4.) * s);
            path.line_to((cx - 4.) * s, (cy + 4.) * s);
        }
        Icon::Maximize => {
            path.move_to((cx - 4.) * s, (cy + 2.) * s);
            path.line_to(cx * s, (cy - 2.) * s);
            path.line_to((cx + 4.) * s, (cy + 2.) * s);
        }
        Icon::Minimize => {
            path.move_to((cx - 4.) * s, (cy - 3.) * s);
            path.line_to(cx * s, (cy + 1.) * s);
            path.line_to((cx + 4.) * s, (cy - 3.) * s);
        }
    }
    pixmap.stroke_path(
        &path.finish().unwrap(),
        &icon_color,
        &stroke,
        Transform::identity(),
        None,
    );
}
