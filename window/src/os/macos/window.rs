// let () = msg_send! is a common pattern for objc
#![allow(clippy::let_unit_value)]

use super::keycodes::*;
use super::{nsstring, nsstring_to_str};
use crate::connection::ConnectionOps;
use crate::parameters::{Border, Parameters, TitleBar};
use crate::{
    Clipboard, Connection, DeadKeyStatus, Dimensions, Handled, KeyCode, KeyEvent, Modifiers,
    MouseButtons, MouseCursor, MouseEvent, MouseEventKind, MousePress, Point, RawKeyEvent, Rect,
    RequestedWindowGeometry, ResolvedGeometry, ScreenPoint, Size, ULength, WindowDecorations,
    WindowEvent, WindowEventSender, WindowOps, WindowState,
};
use anyhow::{anyhow, bail, ensure};
use async_trait::async_trait;
use clipboard_macos::Clipboard as ClipboardContext;
use cocoa::appkit::{
    self, CGFloat, NSApplication, NSApplicationActivateIgnoringOtherApps,
    NSApplicationPresentationOptions, NSBackingStoreBuffered, NSEvent, NSEventModifierFlags,
    NSOpenGLContext, NSOpenGLPixelFormat, NSPasteboard, NSRunningApplication, NSScreen, NSView,
    NSViewHeightSizable, NSViewWidthSizable, NSWindow, NSWindowStyleMask,
};
use cocoa::base::*;
use cocoa::foundation::{
    NSArray, NSAutoreleasePool, NSFastEnumeration, NSInteger, NSNotFound, NSPoint, NSRect, NSSize,
    NSUInteger,
};
use config::ConfigHandle;
use core_foundation::base::{CFTypeID, TCFType};
use core_foundation::bundle::{CFBundleGetBundleWithIdentifier, CFBundleGetFunctionPointerForName};
use core_foundation::data::{CFData, CFDataGetBytePtr, CFDataRef};
use core_foundation::string::{CFString, CFStringRef, UniChar};
use core_foundation::{declare_TCFType, impl_TCFType};
use objc::declare::ClassDecl;
use objc::rc::{StrongPtr, WeakPtr};
use objc::runtime::{Class, Object, Protocol, Sel};
use objc::*;
use promise::Future;
use raw_window_handle::{AppKitWindowHandle, HasRawWindowHandle, RawWindowHandle};
use std::any::Any;
use std::cell::RefCell;
use std::ffi::c_void;
use std::path::PathBuf;
use std::rc::Rc;
use std::str::FromStr;
use std::time::Instant;
use wezterm_font::FontConfiguration;
use wezterm_input_types::is_ascii_control;

#[allow(non_upper_case_globals)]
const NSViewLayerContentsPlacementTopLeft: NSInteger = 11;

fn round_away_from_zerof(value: f64) -> f64 {
    if value > 0. {
        value.max(1.).round()
    } else {
        value.min(-1.).round()
    }
}

fn round_away_from_zero(value: f64) -> i16 {
    if value > 0. {
        value.max(1.).round() as i16
    } else {
        value.min(-1.).round() as i16
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum ImeDisposition {
    /// Nothing happened
    None,
    /// IME triggered an action
    Acted,
    /// We decided to continue with key dispatch
    Continue,
}

#[repr(C)]
struct NSRange(cocoa::foundation::NSRange);

#[derive(Debug)]
#[repr(C)]
struct NSRangePointer(*mut NSRange);

impl std::fmt::Debug for NSRange {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::result::Result<(), std::fmt::Error> {
        fmt.debug_struct("NSRange")
            .field("location", &self.0.location)
            .field("length", &self.0.length)
            .finish()
    }
}

unsafe impl objc::Encode for NSRange {
    fn encode() -> objc::Encoding {
        let encoding = format!(
            "{{NSRange={}{}}}",
            NSUInteger::encode().as_str(),
            NSUInteger::encode().as_str()
        );
        unsafe { objc::Encoding::from_str(&encoding) }
    }
}

unsafe impl objc::Encode for NSRangePointer {
    fn encode() -> objc::Encoding {
        unsafe { objc::Encoding::from_str(&format!("^{}", NSRange::encode().as_str())) }
    }
}

impl NSRange {
    fn new(location: u64, length: u64) -> Self {
        Self(cocoa::foundation::NSRange { location, length })
    }
}

#[derive(Clone)]
pub enum BackendImpl {
    Cgl(Rc<cglbits::GlState>),
    Egl(Rc<crate::egl::GlState>),
}

impl BackendImpl {
    pub fn update(&self) {
        if let Self::Cgl(be) = self {
            be.update();
        }
    }
}

#[derive(Clone)]
pub struct GlContextPair {
    pub context: Rc<glium::backend::Context>,
    pub backend: BackendImpl,
}

impl GlContextPair {
    /// on macOS we first try to initialize EGL by dynamically loading it.
    /// The system doesn't provide an EGL implementation, but the ANGLE
    /// project (and MetalANGLE) both provide implementations.
    /// The ANGLE EGL implementation wants a CALayer descendant passed
    /// as the EGLNativeWindowType.
    pub fn create(view: id) -> anyhow::Result<Self> {
        let behavior = if cfg!(debug_assertions) {
            glium::debug::DebugCallbackBehavior::DebugMessageOnError
        } else {
            glium::debug::DebugCallbackBehavior::Ignore
        };

        // Let's first try to initialize EGL...
        let (context, backend) = match if config::configuration().prefer_egl {
            // ANGLE wants a layer, so tell the view to create one.
            // Importantly, we must set its scale to 1.0 prior to initializing
            // EGL to prevent undesirable scaling.
            let layer: id;
            unsafe {
                let _: () = msg_send![view, setWantsLayer: YES];
                layer = msg_send![view, layer];
                let _: () = msg_send![layer, setContentsScale: 1.0f64];
                let _: () = msg_send![layer, setOpaque: NO];
            };

            let conn = Connection::get().unwrap();

            let state = match conn.gl_connection.borrow().as_ref() {
                None => crate::egl::GlState::create(None, layer as *const c_void),
                Some(glconn) => crate::egl::GlState::create_with_existing_connection(
                    glconn,
                    layer as *const c_void,
                ),
            };

            if state.is_ok() {
                conn.gl_connection
                    .borrow_mut()
                    .replace(Rc::clone(state.as_ref().unwrap().get_connection()));

                // ANGLE will create a CAMetalLayer as a sublayer of our provided
                // layer.  Even though CALayer defaults to !opaque, CAMetalLayer
                // defaults to opaque, so we need to find that layer and fix
                // the opacity so that our alpha values are respected.
                unsafe {
                    let sublayers: id = msg_send![layer, sublayers];
                    let layer_count = sublayers.count();
                    for i in 0..layer_count {
                        let layer = sublayers.objectAtIndex(i);
                        let _: () = msg_send![layer, setOpaque: NO];
                    }
                }
            }

            state
        } else {
            Err(anyhow!("prefers not to use EGL"))
        } {
            Ok(backend) => {
                let backend = Rc::new(backend);
                let context =
                    unsafe { glium::backend::Context::new(Rc::clone(&backend), true, behavior) }?;
                (context, BackendImpl::Egl(backend))
            }
            // ... and then fallback to the deprecated platform provided CGL
            Err(err) => {
                log::debug!("EGL init failed: {:#}, falling back to CGL", err);
                let backend = Rc::new(cglbits::GlState::create(view)?);
                let context =
                    unsafe { glium::backend::Context::new(Rc::clone(&backend), true, behavior) }?;
                (context, BackendImpl::Cgl(backend))
            }
        };

        Ok(Self { context, backend })
    }
}

mod cglbits {
    use super::*;

    pub struct GlState {
        _pixel_format: StrongPtr,
        gl_context: StrongPtr,
    }

    impl GlState {
        pub fn create(view: id) -> anyhow::Result<Self> {
            log::trace!("Calling NSOpenGLPixelFormat::initWithAttributes");
            let pixel_format = unsafe {
                StrongPtr::new(NSOpenGLPixelFormat::alloc(nil).initWithAttributes_(&[
                    appkit::NSOpenGLPFAOpenGLProfile as u32,
                    appkit::NSOpenGLProfileVersion3_2Core as u32,
                    appkit::NSOpenGLPFAClosestPolicy as u32,
                    appkit::NSOpenGLPFAColorSize as u32,
                    32,
                    appkit::NSOpenGLPFAAlphaSize as u32,
                    8,
                    appkit::NSOpenGLPFADepthSize as u32,
                    24,
                    appkit::NSOpenGLPFAStencilSize as u32,
                    8,
                    appkit::NSOpenGLPFAAllowOfflineRenderers as u32,
                    appkit::NSOpenGLPFAAccelerated as u32,
                    appkit::NSOpenGLPFADoubleBuffer as u32,
                    0,
                ]))
            };
            log::trace!("NSOpenGLPixelFormat::initWithAttributes returned");
            ensure!(
                !pixel_format.is_null(),
                "failed to create NSOpenGLPixelFormat"
            );

            // Allow using retina resolutions; without this we're forced into low res
            // and the system will scale us up, resulting in blurry rendering
            unsafe {
                let _: () = msg_send![view, setWantsBestResolutionOpenGLSurface: YES];
            }

            let gl_context = unsafe {
                StrongPtr::new(
                    NSOpenGLContext::alloc(nil).initWithFormat_shareContext_(*pixel_format, nil),
                )
            };
            ensure!(!gl_context.is_null(), "failed to create NSOpenGLContext");

            unsafe {
                let opaque: cgl::GLint = 0;
                gl_context.setValues_forParameter_(
                    &opaque,
                    cocoa::appkit::NSOpenGLContextParameter::NSOpenGLCPSurfaceOpacity,
                );

                gl_context.setView_(view);

                // Explicitly disable vsync; we'll manage throttling frames at
                // the application level
                let swap_interval: cgl::GLint = 0;
                gl_context.setValues_forParameter_(
                    &swap_interval,
                    cocoa::appkit::NSOpenGLContextParameter::NSOpenGLCPSwapInterval,
                );
            }

            Ok(Self {
                _pixel_format: pixel_format,
                gl_context,
            })
        }

        /// Calls NSOpenGLContext update; we need to do this on resize
        pub fn update(&self) {
            unsafe {
                let _: () = msg_send![*self.gl_context, update];
            }
        }
    }

    unsafe impl glium::backend::Backend for GlState {
        fn swap_buffers(&self) -> Result<(), glium::SwapBuffersError> {
            unsafe {
                let pool = NSAutoreleasePool::new(nil);
                self.gl_context.flushBuffer();
                let _: () = msg_send![pool, release];
            }
            Ok(())
        }

        unsafe fn get_proc_address(&self, symbol: &str) -> *const c_void {
            let symbol_name: CFString = FromStr::from_str(symbol).unwrap();
            let framework_name: CFString = FromStr::from_str("com.apple.opengl").unwrap();
            let framework = CFBundleGetBundleWithIdentifier(framework_name.as_concrete_TypeRef());
            let symbol =
                CFBundleGetFunctionPointerForName(framework, symbol_name.as_concrete_TypeRef());
            symbol as *const _
        }

        fn get_framebuffer_dimensions(&self) -> (u32, u32) {
            unsafe {
                let view = self.gl_context.view();
                let frame = NSView::frame(view);
                let backing_frame = NSView::convertRectToBacking(view, frame);
                (
                    backing_frame.size.width as u32,
                    backing_frame.size.height as u32,
                )
            }
        }

        fn is_current(&self) -> bool {
            unsafe {
                let pool = NSAutoreleasePool::new(nil);
                let current = NSOpenGLContext::currentContext(nil);
                let res = if current != nil {
                    let is_equal: BOOL = msg_send![current, isEqual: *self.gl_context];
                    is_equal != NO
                } else {
                    false
                };
                let _: () = msg_send![pool, release];
                res
            }
        }

        unsafe fn make_current(&self) {
            let _: () = msg_send![*self.gl_context, update];
            self.gl_context.makeCurrentContext();
        }
    }
}

pub(crate) struct WindowInner {
    view: StrongPtr,
    window: StrongPtr,
    config: ConfigHandle,
}

fn function_key_to_keycode(function_key: char) -> KeyCode {
    // FIXME: CTRL-C is 0x3, should it be normalized to C here
    // using the unmod string?  Or should be normalize the 0x3
    // as the canonical representation of that input?
    match function_key as u16 {
        appkit::NSUpArrowFunctionKey => KeyCode::UpArrow,
        appkit::NSDownArrowFunctionKey => KeyCode::DownArrow,
        appkit::NSLeftArrowFunctionKey => KeyCode::LeftArrow,
        appkit::NSRightArrowFunctionKey => KeyCode::RightArrow,
        appkit::NSHomeFunctionKey => KeyCode::Home,
        appkit::NSEndFunctionKey => KeyCode::End,
        appkit::NSPageUpFunctionKey => KeyCode::PageUp,
        appkit::NSPageDownFunctionKey => KeyCode::PageDown,
        value @ appkit::NSF1FunctionKey..=appkit::NSF35FunctionKey => {
            KeyCode::Function((value - appkit::NSF1FunctionKey + 1) as u8)
        }
        appkit::NSInsertFunctionKey => KeyCode::Insert,
        appkit::NSDeleteFunctionKey => KeyCode::Char('\u{7f}'),
        appkit::NSPrintScreenFunctionKey => KeyCode::PrintScreen,
        appkit::NSScrollLockFunctionKey => KeyCode::ScrollLock,
        appkit::NSPauseFunctionKey => KeyCode::Pause,
        appkit::NSBreakFunctionKey => KeyCode::Cancel,
        appkit::NSPrintFunctionKey => KeyCode::Print,
        _ => KeyCode::Char(function_key),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct Window {
    id: usize,
    ns_window: *mut Object,
    ns_view: *mut Object,
}

unsafe impl Send for Window {}
unsafe impl Sync for Window {}

impl Window {
    pub async fn new_window<F>(
        _class_name: &str,
        name: &str,
        geometry: RequestedWindowGeometry,
        config: Option<&ConfigHandle>,
        _font_config: Rc<FontConfiguration>,
        event_handler: F,
    ) -> anyhow::Result<Window>
    where
        F: 'static + FnMut(WindowEvent, &Window),
    {
        let config = match config {
            Some(c) => c.clone(),
            None => config::configuration(),
        };

        let conn = Connection::get().expect("new_window called on gui thread");
        let ResolvedGeometry {
            width,
            height,
            x,
            y,
        } = conn.resolve_geometry(geometry);
        let pos = match (x, y) {
            (Some(x), Some(y)) => Some(screen_point_to_cartesian(ScreenPoint::new(
                x as isize, y as isize,
            ))),
            _ => None,
        };

        unsafe {
            let style_mask = decoration_to_mask(config.window_decorations);
            let rect = NSRect::new(
                NSPoint::new(0., 0.),
                NSSize::new(width as f64, height as f64),
            );

            let conn = Connection::get().expect("Connection::init has not been called");

            let window_id = conn.next_window_id();
            let events = WindowEventSender::new(event_handler);

            let inner = Rc::new(RefCell::new(Inner {
                events,
                view_id: None,
                window_id,
                window: None,
                screen_changed: false,
                paint_throttled: false,
                invalidated: true,
                gl_context_pair: None,
                text_cursor_position: Rect::new(Point::new(0, 0), Size::new(0, 0)),
                tracking_rect_tag: 0,
                hscroll_remainder: 0.,
                vscroll_remainder: 0.,
                last_wheel: Instant::now(),
                key_is_down: None,
                dead_pending: None,
                fullscreen: None,
                config: config.clone(),
                ime_state: ImeDisposition::None,
                ime_last_event: None,
                live_resizing: false,
                ime_text: String::new(),
            }));

            let window: id = msg_send![get_window_class(), alloc];
            let window = StrongPtr::new(NSWindow::initWithContentRect_styleMask_backing_defer_(
                window,
                rect,
                style_mask,
                NSBackingStoreBuffered,
                NO,
            ));

            apply_decorations_to_window(&window, config.window_decorations);

            // Prevent Cocoa native tabs from being used
            let _: () = msg_send![*window, setTabbingMode:2 /* NSWindowTabbingModeDisallowed */];
            let _: () = msg_send![*window, setRestorable: NO];

            window.setReleasedWhenClosed_(NO);
            let ns_color: id = msg_send![Class::get("NSColor").unwrap(), alloc];
            window.setBackgroundColor_(cocoa::appkit::NSColor::clearColor(ns_color));

            // We could set this, but it makes the entire window, including
            // its titlebar, opaque to this fixed degree.
            // window.setAlphaValue_(0.4);

            // Window positioning: the first window opens up in the center of
            // the screen.  Subsequent windows will be offset from the position
            // of the prior window at the time it was created.  It's not a
            // perfect algorithm by any means, and doesn't take in account
            // windows moving and closing since the last creation, but it is
            // better than creating them all centered which is what we used
            // to do here.
            thread_local! {
                static LAST_POSITION: RefCell<Option<NSPoint>> = RefCell::new(None);
            }
            LAST_POSITION.with(|last_pos| {
                let pos = pos.or_else(|| last_pos.borrow_mut().take());
                let next_pos = if let Some(pos) = pos {
                    window.cascadeTopLeftFromPoint_(pos)
                } else {
                    window.center();
                    window.cascadeTopLeftFromPoint_(NSPoint::new(0., 0.))
                };
                last_pos.borrow_mut().replace(next_pos);
            });

            window.setTitle_(*nsstring(&name));
            window.setAcceptsMouseMovedEvents_(YES);

            let view = WindowView::alloc(&inner)?;
            view.initWithFrame_(rect);
            view.setAutoresizingMask_(NSViewHeightSizable | NSViewWidthSizable);

            let () = msg_send![
                *view,
                setLayerContentsPlacement: NSViewLayerContentsPlacementTopLeft
            ];

            window.setContentView_(*view);
            window.setDelegate_(*view);

            // register for drag and drop operations.
            let () = msg_send![
                *window,
                registerForDraggedTypes:
                    NSArray::arrayWithObject(nil, appkit::NSFilenamesPboardType)
            ];

            let frame = NSView::frame(*view);
            let backing_frame = NSView::convertRectToBacking(*view, frame);
            let width = backing_frame.size.width;
            let height = backing_frame.size.height;

            let weak_window = window.weak();
            let window_handle = Window {
                id: window_id,
                ns_window: *window,
                ns_view: *view,
            };
            let window_inner = Rc::new(RefCell::new(WindowInner {
                window,
                view,
                config: config.clone(),
            }));
            inner.borrow_mut().window.replace(weak_window);
            conn.windows
                .borrow_mut()
                .insert(window_id, Rc::clone(&window_inner));

            inner
                .borrow_mut()
                .events
                .assign_window(window_handle.clone());

            window_handle.config_did_change(&config);

            // Synthesize a resize event immediately; this allows
            // the embedding application an opportunity to discover
            // the dpi and adjust for display scaling
            inner.borrow_mut().events.dispatch(WindowEvent::Resized {
                dimensions: Dimensions {
                    pixel_width: width as usize,
                    pixel_height: height as usize,
                    dpi: (crate::DEFAULT_DPI * (backing_frame.size.width / frame.size.width))
                        as usize,
                },
                window_state: WindowState::default(),
                live_resizing: false,
            });

            Ok(window_handle)
        }
    }
}

unsafe impl HasRawWindowHandle for Window {
    fn raw_window_handle(&self) -> RawWindowHandle {
        let mut handle = AppKitWindowHandle::empty();
        handle.ns_window = self.ns_window as *mut _;
        handle.ns_view = self.ns_view as *mut _;
        RawWindowHandle::AppKit(handle)
    }
}

#[async_trait(?Send)]
impl WindowOps for Window {
    async fn enable_opengl(&self) -> anyhow::Result<Rc<glium::backend::Context>> {
        let window_id = self.id;
        promise::spawn::spawn(async move {
            if let Some(handle) = Connection::get().unwrap().window_by_id(window_id) {
                let mut inner = handle.borrow_mut();
                inner.enable_opengl()
            } else {
                bail!("invalid window");
            }
        })
        .await
    }

    fn notify<T: Any + Send + Sync>(&self, t: T)
    where
        Self: Sized,
    {
        Connection::with_window_inner(self.id, move |inner| {
            if let Some(window_view) = WindowView::get_this(unsafe { &**inner.view }) {
                window_view
                    .inner
                    .borrow_mut()
                    .events
                    .dispatch(WindowEvent::Notification(Box::new(t)));
            }
            Ok(())
        });
    }

    fn close(&self) {
        Connection::with_window_inner(self.id, |inner| {
            inner.close();
            Ok(())
        });
    }

    fn hide(&self) {
        Connection::with_window_inner(self.id, |inner| {
            inner.hide();
            Ok(())
        });
    }

    fn show(&self) {
        Connection::with_window_inner(self.id, |inner| {
            inner.show();
            Ok(())
        });
    }

    fn set_cursor(&self, cursor: Option<MouseCursor>) {
        Connection::with_window_inner(self.id, move |inner| {
            let _ = inner.set_cursor(cursor);
            Ok(())
        });
    }

    fn invalidate(&self) {
        Connection::with_window_inner(self.id, |inner| {
            inner.invalidate();
            Ok(())
        });
    }

    fn set_title(&self, title: &str) {
        let title = title.to_owned();
        Connection::with_window_inner(self.id, move |inner| {
            inner.set_title(&title);
            Ok(())
        });
    }

    fn set_inner_size(&self, width: usize, height: usize) {
        Connection::with_window_inner(
            self.id,
            move |inner| Ok(inner.set_inner_size(width, height)),
        );
    }

    fn set_window_position(&self, coords: ScreenPoint) {
        Connection::with_window_inner(self.id, move |inner| {
            inner.set_window_position(coords);
            Ok(())
        });
    }

    fn set_text_cursor_position(&self, cursor: Rect) {
        Connection::with_window_inner(self.id, move |inner| {
            inner.set_text_cursor_position(cursor);
            Ok(())
        });
    }

    fn get_clipboard(&self, _clipboard: Clipboard) -> Future<String> {
        Future::result(
            ClipboardContext::new()
                .and_then(|ctx| ctx.read())
                .map_err(|e| anyhow!("Failed to get clipboard:{}", e)),
        )
    }

    fn set_clipboard(&self, _clipboard: Clipboard, text: String) {
        ClipboardContext::new()
            .and_then(|mut ctx| ctx.write(text))
            .ok();
    }

    fn toggle_fullscreen(&self) {
        Connection::with_window_inner(self.id, move |inner| {
            inner.toggle_fullscreen();
            Ok(())
        });
    }

    fn maximize(&self) {
        Connection::with_window_inner(self.id, move |inner| {
            inner.maximize();
            Ok(())
        });
    }

    fn restore(&self) {
        Connection::with_window_inner(self.id, move |inner| {
            inner.restore();
            Ok(())
        });
    }

    fn set_resize_increments(&self, x: u16, y: u16) {
        Connection::with_window_inner(self.id, move |inner| {
            inner.set_resize_increments(x, y);
            Ok(())
        });
    }

    fn config_did_change(&self, config: &ConfigHandle) {
        let config = config.clone();
        Connection::with_window_inner(self.id, move |inner| {
            inner.config_did_change(&config);
            Ok(())
        });
    }

    fn get_os_parameters(
        &self,
        _config: &ConfigHandle,
        window_state: WindowState,
    ) -> anyhow::Result<Option<Parameters>> {
        let raw = self.raw_window_handle();

        // We implement this method primarily to provide Notch-avoidance for
        // systems with a notch.
        // We only need this for non-native full screen mode.

        let native_full_screen = match raw {
            RawWindowHandle::AppKit(raw) => {
                let style_mask = unsafe { NSWindow::styleMask(raw.ns_window as *mut Object) };
                style_mask.contains(NSWindowStyleMask::NSFullScreenWindowMask)
            }
            _ => false,
        };

        let border_dimensions =
            if window_state.contains(WindowState::FULL_SCREEN) && !native_full_screen {
                let main_screen = unsafe { NSScreen::mainScreen(nil) };
                let has_safe_area_insets: BOOL =
                    unsafe { msg_send![main_screen, respondsToSelector: sel!(safeAreaInsets)] };
                if has_safe_area_insets == YES {
                    #[derive(Debug)]
                    struct NSEdgeInsets {
                        top: CGFloat,
                        left: CGFloat,
                        bottom: CGFloat,
                        right: CGFloat,
                    }
                    let insets: NSEdgeInsets = unsafe { msg_send![main_screen, safeAreaInsets] };
                    log::trace!("{:?}", insets);

                    // Bleh, the API is supposed to give us the right metrics, but it needs
                    // a tweak to look good around the notch.
                    // <https://github.com/wez/wezterm/issues/1737#issuecomment-1085923867>
                    let top = insets.top.ceil() as usize;
                    let top = if top > 0 { top + 2 } else { 0 };
                    Some(Border {
                        top: ULength::new(top),
                        left: ULength::new(insets.left.ceil() as usize),
                        right: ULength::new(insets.right.ceil() as usize),
                        bottom: ULength::new(insets.bottom.ceil() as usize),
                        color: crate::color::LinearRgba::with_components(0., 0., 0., 1.),
                    })
                } else {
                    None
                }
            } else {
                None
            };

        Ok(Some(Parameters {
            title_bar: TitleBar {
                padding_left: ULength::new(0),
                padding_right: ULength::new(0),
                height: None,
                font_and_size: None,
            },
            border_dimensions,
        }))
    }
}

/// Convert from a macOS screen coordinate with the origin in the bottom left
/// to a pixel coordinate with its origin in the top left
fn cartesian_to_screen_point(cartesian: NSPoint) -> ScreenPoint {
    unsafe {
        let screens = NSScreen::screens(nil);
        let primary = screens.objectAtIndex(0);
        let frame = NSScreen::frame(primary);
        let backing_frame = NSScreen::convertRectToBacking_(primary, frame);
        let scale = backing_frame.size.height / frame.size.height;
        ScreenPoint::new(
            (cartesian.x * scale) as isize,
            ((frame.size.height - cartesian.y) * scale) as isize,
        )
    }
}

/// Convert from a pixel coordinate in the top left to a macOS screen
/// coordinate with its origin in the bottom left
fn screen_point_to_cartesian(point: ScreenPoint) -> NSPoint {
    unsafe {
        let screens = NSScreen::screens(nil);
        let primary = screens.objectAtIndex(0);
        let frame = NSScreen::frame(primary);
        let backing_frame = NSScreen::convertRectToBacking_(primary, frame);
        let scale = backing_frame.size.height / frame.size.height;
        NSPoint::new(
            point.x as f64 / scale,
            frame.size.height - (point.y as f64 / scale),
        )
    }
}

impl WindowInner {
    fn enable_opengl(&mut self) -> anyhow::Result<Rc<glium::backend::Context>> {
        if let Some(window_view) = WindowView::get_this(unsafe { &**self.view }) {
            window_view.inner.borrow_mut().enable_opengl()
        } else {
            anyhow::bail!("window invalid");
        }
    }

    fn is_fullscreen(&mut self) -> bool {
        if self.is_native_fullscreen() {
            true
        } else if let Some(window_view) = WindowView::get_this(unsafe { &**self.view }) {
            window_view.inner.borrow().fullscreen.is_some()
        } else {
            false
        }
    }

    fn apply_decorations(&mut self) {
        if !self.is_fullscreen() {
            apply_decorations_to_window(&self.window, self.config.window_decorations);
        }
    }

    fn toggle_native_fullscreen(&mut self) {
        unsafe {
            NSWindow::toggleFullScreen_(*self.window, nil);
        }
    }

    fn is_native_fullscreen(&self) -> bool {
        let style_mask = unsafe { NSWindow::styleMask(*self.window) };
        style_mask.contains(NSWindowStyleMask::NSFullScreenWindowMask)
    }

    /// If we were in native full screen mode, exit it and return true.
    /// Otherwise, return false
    fn exit_native_fullscreen(&mut self) -> bool {
        if self.is_native_fullscreen() {
            self.toggle_native_fullscreen();
            true
        } else {
            false
        }
    }

    /// If we were in simple full screen mode, exit it and return true.
    /// Otherwise, return false
    fn exit_simple_fullscreen(&mut self) -> bool {
        if let Some(window_view) = WindowView::get_this(unsafe { &**self.view }) {
            let is_fullscreen = window_view.inner.borrow().fullscreen.is_some();
            if is_fullscreen {
                self.toggle_simple_fullscreen();
            }
            is_fullscreen
        } else {
            false
        }
    }

    fn toggle_simple_fullscreen(&mut self) {
        let current_app = unsafe { NSApplication::sharedApplication(nil) };

        if let Some(window_view) = WindowView::get_this(unsafe { &**self.view }) {
            let fullscreen = window_view.inner.borrow_mut().fullscreen.take();
            match fullscreen {
                Some(saved_rect) => unsafe {
                    // Restore prior dimensions
                    self.window.orderOut_(nil);
                    apply_decorations_to_window(&self.window, self.config.window_decorations);
                    self.window.setFrame_display_(saved_rect, YES);
                    self.window.makeKeyAndOrderFront_(nil);
                    self.window.setOpaque_(NO);
                    current_app.setPresentationOptions_(
                        NSApplicationPresentationOptions::NSApplicationPresentationDefault,
                    );
                },
                None => unsafe {
                    // Go full screen
                    let saved_rect = NSWindow::frame(*self.window);
                    window_view
                        .inner
                        .borrow_mut()
                        .fullscreen
                        .replace(saved_rect);

                    let main_screen = NSScreen::mainScreen(nil);
                    let screen_rect = NSScreen::frame(main_screen);

                    self.window.orderOut_(nil);
                    self.window
                        .setStyleMask_(NSWindowStyleMask::NSBorderlessWindowMask);
                    self.window.setFrame_display_(screen_rect, YES);
                    self.window.makeKeyAndOrderFront_(nil);
                    self.window.setOpaque_(YES);
                    current_app.setPresentationOptions_(
                        NSApplicationPresentationOptions:: NSApplicationPresentationAutoHideMenuBar
                            | NSApplicationPresentationOptions::NSApplicationPresentationAutoHideDock
                    );
                },
            }
        }
    }

    fn update_window_shadow(&mut self) {
        let is_opaque = if self.config.window_background_opacity >= 1.0 {
            YES
        } else {
            NO
        };
        unsafe {
            self.window.setOpaque_(is_opaque);
            // when transparent, also turn off the window shadow,
            // because having the shadow enabled seems to correlate
            // with ghostly remnants see:
            // https://github.com/wez/wezterm/issues/310
            self.window.setHasShadow_(is_opaque);
        }
    }
}

impl WindowInner {
    fn show(&mut self) {
        unsafe {
            let current_app = NSRunningApplication::currentApplication(nil);
            current_app.activateWithOptions_(NSApplicationActivateIgnoringOtherApps);
            self.window.makeKeyAndOrderFront_(nil)
        }
    }

    fn close(&mut self) {
        unsafe {
            self.window.close();
        }
    }

    fn hide(&mut self) {
        unsafe {
            NSWindow::miniaturize_(*self.window, *self.window);
            // We could literally set it invisible like this, but
            // then there is no UI to make it visible again later.
            //let () = msg_send![*self.window, setIsVisible: NO];
        }
    }

    fn set_cursor(&mut self, cursor: Option<MouseCursor>) {
        unsafe {
            let ns_cursor_cls = class!(NSCursor);
            if let Some(cursor) = cursor {
                // Unconditionally apply the requested cursor, as there are
                // cases where macOS can decide to change the cursor to something
                // that we don't know about.
                let instance: id = match cursor {
                    MouseCursor::Arrow => msg_send![ns_cursor_cls, arrowCursor],
                    MouseCursor::Text => msg_send![ns_cursor_cls, IBeamCursor],
                    MouseCursor::Hand => msg_send![ns_cursor_cls, pointingHandCursor],
                    MouseCursor::SizeUpDown => msg_send![ns_cursor_cls, resizeUpDownCursor],
                    MouseCursor::SizeLeftRight => msg_send![ns_cursor_cls, resizeLeftRightCursor],
                };
                let () = msg_send![ns_cursor_cls, setHiddenUntilMouseMoves: NO];
                let () = msg_send![instance, set];
            } else {
                let () = msg_send![ns_cursor_cls, setHiddenUntilMouseMoves: YES];
            }
        }
    }

    fn invalidate(&mut self) {
        unsafe {
            let () = msg_send![*self.view, setNeedsDisplay: YES];
            if let Some(window_view) = WindowView::get_this(&**self.view) {
                window_view.inner.borrow_mut().invalidated = true;
            }
        }
    }
    fn set_title(&mut self, title: &str) {
        let title = nsstring(title);
        unsafe {
            NSWindow::setTitle_(*self.window, *title);
        }
    }

    fn set_inner_size(&mut self, width: usize, height: usize) -> Dimensions {
        unsafe {
            let frame = NSView::frame(*self.view as *mut _);
            let backing_frame = NSView::convertRectToBacking(*self.view as *mut _, frame);
            let scale = backing_frame.size.width / frame.size.width;

            NSWindow::setContentSize_(
                *self.window,
                NSSize::new(width as f64 / scale, height as f64 / scale),
            );

            let frame = NSView::frame(*self.view as *mut _);
            let backing_frame = NSView::convertRectToBacking(*self.view as *mut _, frame);
            let width = backing_frame.size.width;
            let height = backing_frame.size.height;
            Dimensions {
                pixel_width: width as usize,
                pixel_height: height as usize,
                dpi: (crate::DEFAULT_DPI * (backing_frame.size.width / frame.size.width)) as usize,
            }
        }
    }

    fn set_window_position(&self, coords: ScreenPoint) {
        unsafe {
            let cartesian = screen_point_to_cartesian(coords);
            let frame = NSWindow::frame(*self.window);
            let content_frame = NSWindow::contentRectForFrameRect_(*self.window, frame);
            let delta_x = content_frame.origin.x - frame.origin.x;
            let delta_y = content_frame.origin.y - frame.origin.y;
            let point = NSPoint::new(
                cartesian.x as f64 - delta_x,
                cartesian.y as f64 - delta_y - content_frame.size.height,
            );
            NSWindow::setFrameOrigin_(*self.window, point);
        }
    }

    fn set_text_cursor_position(&mut self, cursor: Rect) {
        if let Some(window_view) = WindowView::get_this(unsafe { &**self.view }) {
            window_view.inner.borrow_mut().text_cursor_position = cursor;
        }
        if self.config.use_ime {
            unsafe {
                let input_context: id = msg_send![&**self.view, inputContext];
                let () = msg_send![input_context, invalidateCharacterCoordinates];
            }
        }
    }

    fn is_zoomed(&self) -> bool {
        unsafe { msg_send![*self.window, isZoomed] }
    }

    fn maximize(&mut self) {
        if !self.is_zoomed() {
            unsafe {
                NSWindow::zoom_(*self.window, nil);
            }
        }
    }

    fn restore(&mut self) {
        if self.is_zoomed() {
            unsafe {
                NSWindow::zoom_(*self.window, nil);
            }
        }
    }

    fn toggle_fullscreen(&mut self) {
        let native_fullscreen = self.config.native_macos_fullscreen_mode;

        // If they changed their config since going full screen, be sure
        // to undo whichever fullscreen mode they had active rather than
        // trying to undo the one they have configured.

        if native_fullscreen {
            if !self.exit_simple_fullscreen() {
                self.toggle_native_fullscreen();
            }
        } else {
            if !self.exit_native_fullscreen() {
                self.toggle_simple_fullscreen();
            }
        }
    }

    fn set_resize_increments(&self, x: u16, y: u16) {
        unsafe {
            self.window
                .setResizeIncrements_(NSSize::new(x.into(), y.into()));
        }
    }

    fn config_did_change(&mut self, config: &ConfigHandle) {
        self.config = config.clone();
        if let Some(window_view) = WindowView::get_this(unsafe { &**self.view }) {
            window_view.inner.borrow_mut().config = config.clone();
        }
        self.update_window_shadow();
        self.apply_decorations();
    }
}

fn apply_decorations_to_window(window: &StrongPtr, decorations: WindowDecorations) {
    let mask = decoration_to_mask(decorations);
    unsafe {
        window.setStyleMask_(mask);

        let hidden = if decorations.contains(WindowDecorations::TITLE) {
            NO
        } else {
            YES
        };

        for titlebar_button in &[
            appkit::NSWindowButton::NSWindowFullScreenButton,
            appkit::NSWindowButton::NSWindowMiniaturizeButton,
            appkit::NSWindowButton::NSWindowCloseButton,
            appkit::NSWindowButton::NSWindowZoomButton,
        ] {
            let button = window.standardWindowButton_(*titlebar_button);
            let _: () = msg_send![button, setHidden: hidden];
        }

        window.setTitleVisibility_(if decorations.contains(WindowDecorations::TITLE) {
            appkit::NSWindowTitleVisibility::NSWindowTitleVisible
        } else {
            appkit::NSWindowTitleVisibility::NSWindowTitleHidden
        });
        window.setTitlebarAppearsTransparent_(hidden);
    }
}

fn decoration_to_mask(decorations: WindowDecorations) -> NSWindowStyleMask {
    if decorations == WindowDecorations::TITLE | WindowDecorations::RESIZE {
        NSWindowStyleMask::NSTitledWindowMask
            | NSWindowStyleMask::NSClosableWindowMask
            | NSWindowStyleMask::NSMiniaturizableWindowMask
            | NSWindowStyleMask::NSResizableWindowMask
    } else if decorations == WindowDecorations::RESIZE {
        NSWindowStyleMask::NSTitledWindowMask
            | NSWindowStyleMask::NSClosableWindowMask
            | NSWindowStyleMask::NSMiniaturizableWindowMask
            | NSWindowStyleMask::NSResizableWindowMask
            | NSWindowStyleMask::NSFullSizeContentViewWindowMask
    } else if decorations == WindowDecorations::NONE {
        NSWindowStyleMask::NSTitledWindowMask
            | NSWindowStyleMask::NSClosableWindowMask
            | NSWindowStyleMask::NSMiniaturizableWindowMask
            | NSWindowStyleMask::NSFullSizeContentViewWindowMask
    } else if decorations == WindowDecorations::TITLE {
        NSWindowStyleMask::NSTitledWindowMask
            | NSWindowStyleMask::NSClosableWindowMask
            | NSWindowStyleMask::NSMiniaturizableWindowMask
    } else {
        NSWindowStyleMask::NSTitledWindowMask
            | NSWindowStyleMask::NSClosableWindowMask
            | NSWindowStyleMask::NSMiniaturizableWindowMask
            | NSWindowStyleMask::NSResizableWindowMask
    }
}

#[derive(Debug)]
struct DeadKeyState {
    /// The private dead key state preserved from UCKeyTranslate
    dead_state: u32,
}

struct Inner {
    events: WindowEventSender,
    view_id: Option<WeakPtr>,
    window: Option<WeakPtr>,
    screen_changed: bool,
    paint_throttled: bool,
    window_id: usize,
    invalidated: bool,
    gl_context_pair: Option<GlContextPair>,
    text_cursor_position: Rect,
    tracking_rect_tag: NSInteger,
    hscroll_remainder: f64,
    vscroll_remainder: f64,
    last_wheel: Instant,
    /// We use this to avoid double-emitting events when
    /// procesing key-up events.
    key_is_down: Option<bool>,

    /// First in a dead-key sequence
    dead_pending: Option<DeadKeyState>,

    /// When using simple fullscreen mode, this tracks
    /// the window dimensions that need to be restored
    fullscreen: Option<NSRect>,

    config: ConfigHandle,

    /// Used to signal when IME really just swallowed a key
    ime_state: ImeDisposition,
    /// Captures the last event that had ImeDisposition::Acted,
    /// so that we can use it to generate a repeat in the cases
    /// where the IME mysteriously swallows repeats but only
    /// for certain keys.
    ime_last_event: Option<KeyEvent>,

    /// Whether we're in live resize
    live_resizing: bool,

    ime_text: String,
}

#[repr(C)]
pub struct __InputSource {
    _dummy: i32,
}
pub type InputSourceRef = *const __InputSource;

declare_TCFType!(InputSource, InputSourceRef);
impl_TCFType!(InputSource, InputSourceRef, TISInputSourceGetTypeID);

#[repr(C)]
struct UCKeyboardLayout {
    _dummy: i32,
}

type UniCharCount = std::os::raw::c_ulong;

/// key is going down
#[allow(non_upper_case_globals)]
const kUCKeyActionDown: u16 = 0;
/// key is going up
#[allow(non_upper_case_globals, dead_code)]
const kUCKeyActionUp: u16 = 1;
/// auto-key down
#[allow(non_upper_case_globals, dead_code)]
const kUCKeyActionAutoKey: u16 = 2;
/// get information for key display (as in Key Caps)
#[allow(non_upper_case_globals)]
const kUCKeyActionDisplay: u16 = 3;

extern "C" {
    fn TISInputSourceGetTypeID() -> CFTypeID;
    fn TISCopyCurrentKeyboardInputSource() -> InputSourceRef;
    fn TISGetInputSourceProperty(source: InputSourceRef, propertyKey: CFStringRef) -> CFDataRef;

    static kTISPropertyUnicodeKeyLayoutData: CFStringRef;

    fn UCKeyTranslate(
        layout: *const UCKeyboardLayout,
        virtualKeyCode: u16,
        keyAction: u16,
        modifierKeyState: u32,
        keyboardType: u32,
        keyTranslateOptions: u32,
        deadKeyState: *mut u32,
        maxStringLength: UniCharCount,
        actualStringLength: *mut UniCharCount,
        unicodeString: *mut UniChar,
    ) -> u32;

    fn LMGetKbdType() -> u8;
}

#[derive(Debug)]
enum TranslateStatus {
    Composing(String),
    Composed(String),
    NotDead,
}

/// Represents the current keyboard layout.
/// Holds state needed to perform keymap translation.
struct Keyboard {
    _kbd: InputSource,
    layout_data: Option<CFData>,
}

/// Slightly more intelligible parameters for keymap translation
struct TranslateParams {
    virtual_key_code: u16,
    modifier_flags: NSEventModifierFlags,
    dead_state: u32,
    ignore_dead_keys: bool,
    display: bool,
}

/// The results of a keymap translation
#[derive(Debug)]
struct TranslateResults {
    dead_state: u32,
    text: String,
}

impl Keyboard {
    pub fn new() -> Self {
        let _kbd =
            unsafe { InputSource::wrap_under_create_rule(TISCopyCurrentKeyboardInputSource()) };

        let layout_data = unsafe {
            let data = TISGetInputSourceProperty(
                _kbd.as_concrete_TypeRef(),
                kTISPropertyUnicodeKeyLayoutData,
            );
            if data.is_null() {
                None
            } else {
                Some(CFData::wrap_under_get_rule(data))
            }
        };
        Self { _kbd, layout_data }
    }

    /// A wrapper around UCKeyTranslate
    pub fn translate(&self, params: TranslateParams) -> anyhow::Result<TranslateResults> {
        let layout_data = match &self.layout_data {
            Some(data) => unsafe {
                CFDataGetBytePtr(data.as_concrete_TypeRef()) as *const UCKeyboardLayout
            },
            None => std::ptr::null(),
        };

        let modifier_key_state: u32 = (params.modifier_flags.bits() >> 16) as u32 & 0xFF;

        let kbd_type = unsafe { LMGetKbdType() } as _;
        #[allow(non_upper_case_globals)]
        const kUCKeyTranslateNoDeadKeysBit: u32 = 0;

        let mut unicode_buffer = [0u16; 32];
        let mut length = 0;
        let mut dead_state = params.dead_state;
        unsafe {
            UCKeyTranslate(
                layout_data,
                params.virtual_key_code,
                if params.display {
                    kUCKeyActionDisplay
                } else {
                    kUCKeyActionDown
                },
                modifier_key_state,
                kbd_type,
                if params.ignore_dead_keys {
                    1 << kUCKeyTranslateNoDeadKeysBit
                } else {
                    0
                },
                &mut dead_state,
                unicode_buffer.len() as _,
                &mut length,
                unicode_buffer.as_mut_ptr(),
            )
        };

        let text = String::from_utf16(unsafe {
            std::slice::from_raw_parts(unicode_buffer.as_mut_ptr(), length as _)
        })?;

        Ok(TranslateResults { text, dead_state })
    }
}

impl Inner {
    fn enable_opengl(&mut self) -> anyhow::Result<Rc<glium::backend::Context>> {
        let view = self.view_id.as_ref().unwrap().load();
        let glium_context = GlContextPair::create(*view)?;

        self.gl_context_pair.replace(glium_context.clone());

        Ok(glium_context.context)
    }

    /// <https://stackoverflow.com/a/22677690>
    /// <https://stackoverflow.com/a/12548163>
    /// <https://stackoverflow.com/a/8263841>
    /// <https://developer.apple.com/documentation/coreservices/1390584-uckeytranslate?language=objc>
    fn translate_key_event(
        &mut self,
        virtual_key_code: u16,
        modifier_flags: NSEventModifierFlags,
    ) -> anyhow::Result<TranslateStatus> {
        let keyboard = Keyboard::new();

        let mods = key_modifiers(modifier_flags);

        let config = &self.config;

        let use_dead_keys = if !config.use_dead_keys {
            false
        } else if mods.contains(Modifiers::LEFT_ALT) {
            config.send_composed_key_when_left_alt_is_pressed
        } else if mods.contains(Modifiers::RIGHT_ALT) {
            config.send_composed_key_when_right_alt_is_pressed
        } else {
            true
        };

        if let Some(DeadKeyState { dead_state }) = self.dead_pending.take() {
            let result = keyboard.translate(TranslateParams {
                virtual_key_code,
                modifier_flags,
                dead_state,
                ignore_dead_keys: false,
                display: true,
            })?;

            // If length == 0 it means that they double-pressed the dead key.
            // We treat that the same as the dead key disabled state:
            // we want to clock through a space keypress so that we clear
            // the state and output the original keypress.
            let generate_space = !use_dead_keys || result.text.len() == 0;

            if generate_space {
                // synthesize a SPACE press to
                // elicit the underlying key code and get out
                // of the dead key state
                let result = keyboard.translate(TranslateParams {
                    virtual_key_code,
                    modifier_flags,
                    dead_state: result.dead_state,
                    ignore_dead_keys: false,
                    display: false,
                })?;
                Ok(TranslateStatus::Composed(result.text))
            } else {
                Ok(TranslateStatus::Composed(result.text))
            }
        } else if use_dead_keys {
            let result = keyboard.translate(TranslateParams {
                virtual_key_code,
                modifier_flags,
                dead_state: 0,
                ignore_dead_keys: false,
                display: false,
            })?;

            self.dead_pending.replace(DeadKeyState {
                dead_state: result.dead_state,
            });

            // Get the non-dead-key rendition to show as the composing state
            let composing = keyboard.translate(TranslateParams {
                virtual_key_code,
                modifier_flags,
                dead_state: 0,
                ignore_dead_keys: true,
                display: true,
            })?;

            Ok(TranslateStatus::Composing(composing.text))
        } else {
            Ok(TranslateStatus::NotDead)
        }
    }
}

const VIEW_CLS_NAME: &str = "WezTermWindowView";
const WINDOW_CLS_NAME: &str = "WezTermWindow";

struct WindowView {
    inner: Rc<RefCell<Inner>>,
}

pub fn superclass(this: &Object) -> &'static Class {
    unsafe {
        let superclass: id = msg_send![this, superclass];
        &*(superclass as *const _)
    }
}

#[allow(clippy::identity_op)]
fn decode_mouse_buttons(mask: u64) -> MouseButtons {
    let mut buttons = MouseButtons::NONE;

    if (mask & (1 << 0)) != 0 {
        buttons |= MouseButtons::LEFT;
    }
    if (mask & (1 << 1)) != 0 {
        buttons |= MouseButtons::RIGHT;
    }
    if (mask & (1 << 2)) != 0 {
        buttons |= MouseButtons::MIDDLE;
    }
    if (mask & (1 << 3)) != 0 {
        buttons |= MouseButtons::X1;
    }
    if (mask & (1 << 4)) != 0 {
        buttons |= MouseButtons::X2;
    }
    buttons
}

fn key_modifiers(flags: NSEventModifierFlags) -> Modifiers {
    let mut mods = Modifiers::NONE;

    if flags.contains(NSEventModifierFlags::NSShiftKeyMask) {
        mods |= Modifiers::SHIFT;
    }
    if flags.contains(NSEventModifierFlags::NSAlternateKeyMask) && (flags.bits() & 0x20) != 0 {
        mods |= Modifiers::LEFT_ALT | Modifiers::ALT;
    }
    if flags.contains(NSEventModifierFlags::NSAlternateKeyMask) && (flags.bits() & 0x40) != 0 {
        mods |= Modifiers::RIGHT_ALT | Modifiers::ALT;
    }
    if flags.contains(NSEventModifierFlags::NSControlKeyMask) {
        mods |= Modifiers::CTRL;
    }
    if flags.contains(NSEventModifierFlags::NSCommandKeyMask) {
        mods |= Modifiers::SUPER;
    }

    mods
}

/// We register our own subclass of NSWindow so that we can override
/// canBecomeKeyWindow so that our simple fullscreen style can keep
/// focus once the titlebar has been removed; the default behavior of
/// NSWindow is to reject focus when it doesn't have a titlebar!
fn get_window_class() -> &'static Class {
    Class::get(WINDOW_CLS_NAME).unwrap_or_else(|| {
        let mut cls = ClassDecl::new(WINDOW_CLS_NAME, class!(NSWindow))
            .expect("Unable to register Window class");

        extern "C" fn yes(_: &mut Object, _: Sel) -> BOOL {
            YES
        }

        unsafe {
            cls.add_method(
                sel!(canBecomeKeyWindow),
                yes as extern "C" fn(&mut Object, Sel) -> BOOL,
            );
            cls.add_method(
                sel!(canBecomeMainWindow),
                yes as extern "C" fn(&mut Object, Sel) -> BOOL,
            );
        }

        cls.register()
    })
}

impl WindowView {
    extern "C" fn dealloc(this: &mut Object, _sel: Sel) {
        Self::drop_inner(this);
        unsafe {
            let superclass = superclass(this);
            let () = msg_send![super(this, superclass), dealloc];
        }
    }

    /// `dealloc` is called when our NSView descendant is destroyed.
    /// In practice, I've not seen this trigger, which likely means
    /// that there is something afoot with reference counting.
    /// The cardinality of Window and View objects is low enough
    /// that I'm "OK" with this for now.
    /// What really matters is that the `Inner` object is dropped
    /// in a timely fashion once the window is closed, so we manage
    /// that by hooking into `windowWillClose` and routing both
    /// `dealloc` and `windowWillClose` to `drop_inner`.
    fn drop_inner(this: &mut Object) {
        unsafe {
            let myself: *mut c_void = *this.get_ivar(VIEW_CLS_NAME);
            this.set_ivar(VIEW_CLS_NAME, std::ptr::null_mut() as *mut c_void);

            if !myself.is_null() {
                let myself = Box::from_raw(myself as *mut Self);
                drop(myself);
            }
        }
    }

    // Called by the inputContext manager when the IME processes events.
    // We need to translate the selector back into appropriate key
    // sequences
    extern "C" fn do_command_by_selector(this: &mut Object, _sel: Sel, a_selector: Sel) {
        let selector = format!("{:?}", a_selector);
        log::trace!("do_command_by_selector {:?}", selector);

        if let Some(myself) = Self::get_this(this) {
            let mut inner = myself.inner.borrow_mut();
            inner.ime_state = ImeDisposition::Continue;
            inner.ime_last_event.take();
        }
    }

    extern "C" fn has_marked_text(this: &mut Object, _sel: Sel) -> BOOL {
        if let Some(myself) = Self::get_this(this) {
            let inner = myself.inner.borrow();
            if inner.ime_text.is_empty() {
                NO
            } else {
                YES
            }
        } else {
            NO
        }
    }

    extern "C" fn marked_range(this: &mut Object, _sel: Sel) -> NSRange {
        if let Some(myself) = Self::get_this(this) {
            let inner = myself.inner.borrow();
            log::trace!("marked_range {:?}", inner.ime_text);
            if inner.ime_text.is_empty() {
                NSRange::new(NSNotFound as _, 0)
            } else {
                NSRange::new(0, inner.ime_text.len() as u64)
            }
        } else {
            NSRange::new(NSNotFound as _, 0)
        }
    }

    extern "C" fn selected_range(_this: &mut Object, _sel: Sel) -> NSRange {
        NSRange::new(NSNotFound as _, 0)
    }

    // Called by the IME when inserting composed text and/or emoji
    extern "C" fn insert_text_replacement_range(
        this: &mut Object,
        _sel: Sel,
        astring: id,
        replacement_range: NSRange,
    ) {
        let s = unsafe { nsstring_to_str(astring) };
        log::trace!(
            "insert_text_replacement_range {} {:?}",
            s,
            replacement_range
        );
        if let Some(myself) = Self::get_this(this) {
            let mut inner = myself.inner.borrow_mut();

            let key_is_down = inner.key_is_down.take().unwrap_or(true);

            let key = KeyCode::composed(s);

            let event = KeyEvent {
                key,
                modifiers: Modifiers::NONE,
                repeat_count: 1,
                key_is_down,
                raw: None,
            };

            inner.ime_text.clear();
            inner
                .events
                .dispatch(WindowEvent::AdviseDeadKeyStatus(DeadKeyStatus::None));
            inner.ime_last_event.replace(event.clone());
            inner.events.dispatch(WindowEvent::KeyEvent(event));
            inner.ime_state = ImeDisposition::Acted;
        }
    }

    extern "C" fn set_marked_text_selected_range_replacement_range(
        this: &mut Object,
        _sel: Sel,
        astring: id,
        selected_range: NSRange,
        replacement_range: NSRange,
    ) {
        let s = unsafe { nsstring_to_str(astring) };
        log::trace!(
            "set_marked_text_selected_range_replacement_range {} {:?} {:?}",
            s,
            selected_range,
            replacement_range
        );
        if let Some(myself) = Self::get_this(this) {
            let mut inner = myself.inner.borrow_mut();
            inner.ime_text = s.to_string();

            /*
            let key_is_down = inner.key_is_down.take().unwrap_or(true);

            let key = KeyCode::composed(s);

            let event = KeyEvent {
                key,
                modifiers: Modifiers::NONE,
                repeat_count: 1,
                key_is_down,
            }
            .normalize_shift();

            inner.ime_last_event.replace(event.clone());
            inner.events.dispatch(WindowEvent::KeyEvent(event));
            */
            inner.ime_last_event.take();
            inner.ime_state = ImeDisposition::Acted;
        }
    }

    extern "C" fn unmark_text(this: &mut Object, _sel: Sel) {
        log::trace!("unmarkText");
        if let Some(myself) = Self::get_this(this) {
            let mut inner = myself.inner.borrow_mut();
            // FIXME: docs say to insert the text here,
            // but iterm doesn't... and we've never seen
            // this get called so far?
            inner.ime_text.clear();
            inner.ime_last_event.take();
            inner.ime_state = ImeDisposition::Acted;
        }
    }

    extern "C" fn valid_attributes_for_marked_text(_this: &mut Object, _sel: Sel) -> id {
        // FIXME: returns NSArray<NSAttributedStringKey> *
        // log::trace!("valid_attributes_for_marked_text");
        // nil
        unsafe { NSArray::arrayWithObjects(nil, &[]) }
    }

    extern "C" fn attributed_substring_for_proposed_range(
        _this: &mut Object,
        _sel: Sel,
        _proposed_range: NSRange,
        _actual_range: NSRangePointer,
    ) -> id {
        log::trace!(
            "attributedSubstringForProposedRange {:?} {:?}",
            _proposed_range,
            _actual_range
        );
        nil
    }

    extern "C" fn character_index_for_point(
        _this: &mut Object,
        _sel: Sel,
        _point: NSPoint,
    ) -> NSUInteger {
        NSNotFound as _
    }

    extern "C" fn first_rect_for_character_range(
        this: &mut Object,
        _sel: Sel,
        range: NSRange,
        actual: NSRangePointer,
    ) -> NSRect {
        // Returns a rect in screen coordinates; this is used to place
        // the input method editor
        log::trace!(
            "firstRectForCharacterRange: range:{:?} actual:{:?}",
            range,
            actual
        );
        let window: id = unsafe { msg_send![this, window] };
        let frame = unsafe { NSWindow::frame(window) };
        let content: NSRect = unsafe { msg_send![window, contentRectForFrameRect: frame] };
        let backing_frame: NSRect = unsafe { msg_send![this, convertRectToBacking: frame] };
        let scale = frame.size.width / backing_frame.size.width;

        if let Some(this) = Self::get_this(this) {
            let cursor_pos = this
                .inner
                .borrow()
                .text_cursor_position
                .to_f64()
                .scale(scale, scale);

            NSRect::new(
                NSPoint::new(
                    content.origin.x + cursor_pos.min_x(),
                    content.origin.y + content.size.height - cursor_pos.max_y(),
                ),
                NSSize::new(cursor_pos.size.width, cursor_pos.size.height),
            )
        } else {
            frame
        }
    }

    extern "C" fn accepts_first_mouse(_this: &mut Object, _sel: Sel, _nsevent: id) -> BOOL {
        YES
    }

    extern "C" fn accepts_first_responder(_this: &mut Object, _sel: Sel) -> BOOL {
        YES
    }

    extern "C" fn view_did_change_effective_appearance(this: &mut Object, _sel: Sel) {
        if let Some(this) = Self::get_this(this) {
            let appearance = Connection::get().unwrap().get_appearance();
            this.inner
                .borrow_mut()
                .events
                .dispatch(WindowEvent::AppearanceChanged(appearance));
        }
    }

    extern "C" fn update_tracking_areas(this: &mut Object, _sel: Sel) {
        let frame = unsafe { NSView::frame(this as *mut _) };

        if let Some(this) = Self::get_this(this) {
            let mut inner = this.inner.borrow_mut();
            if let Some(ref view) = inner.view_id {
                let view = view.load();
                if view.is_null() {
                    return;
                }

                let tag = inner.tracking_rect_tag;
                if tag != 0 {
                    unsafe {
                        let () = msg_send![*view, removeTrackingRect: tag];
                    }
                }

                let rect = NSRect::new(
                    NSPoint::new(0.0, 0.0),
                    NSSize::new(frame.size.width, frame.size.height),
                );
                inner.tracking_rect_tag = unsafe {
                    msg_send![*view, addTrackingRect: rect owner: *view userData: nil assumeInside: NO]
                };
            }
        }
    }

    extern "C" fn window_should_close(this: &mut Object, _sel: Sel, _id: id) -> BOOL {
        unsafe {
            let () = msg_send![this, setNeedsDisplay: YES];
        }

        if let Some(this) = Self::get_this(this) {
            this.inner
                .borrow_mut()
                .events
                .dispatch(WindowEvent::CloseRequested);
            NO
        } else {
            YES
        }
    }

    extern "C" fn did_become_key(this: &mut Object, _sel: Sel, _id: id) {
        if let Some(this) = Self::get_this(this) {
            this.inner
                .borrow_mut()
                .events
                .dispatch(WindowEvent::FocusChanged(true));
        }
    }

    extern "C" fn did_resign_key(this: &mut Object, _sel: Sel, _id: id) {
        if let Some(this) = Self::get_this(this) {
            this.inner
                .borrow_mut()
                .events
                .dispatch(WindowEvent::FocusChanged(false));
        }
    }

    // Switch the coordinate system to have 0,0 in the top left
    extern "C" fn is_flipped(_this: &Object, _sel: Sel) -> BOOL {
        YES
    }

    // Tell the window/view/layer stuff that we only have a single opaque
    // thing in the window so that it can optimize rendering
    extern "C" fn is_opaque(_this: &Object, _sel: Sel) -> BOOL {
        NO
    }

    // Don't use Cocoa native window tabbing
    extern "C" fn allow_automatic_tabbing(_this: &Object, _sel: Sel) -> BOOL {
        NO
    }

    extern "C" fn window_will_close(this: &mut Object, _sel: Sel, _id: id) {
        if let Some(this) = Self::get_this(this) {
            // Advise the window of its impending death
            this.inner
                .borrow_mut()
                .events
                .dispatch(WindowEvent::Destroyed);
        }

        // Release and zero out the inner member
        Self::drop_inner(this);
    }

    fn mouse_common(this: &mut Object, nsevent: id, kind: MouseEventKind) {
        let view = this as id;
        let coords;
        let mouse_buttons;
        let modifiers;
        let screen_coords;
        unsafe {
            let point = NSView::convertPoint_fromView_(view, nsevent.locationInWindow(), nil);
            let rect = NSRect::new(NSPoint::new(0., 0.), NSSize::new(point.x, point.y));
            let backing_rect = NSView::convertRectToBacking(view, rect);
            // backing_rect computes abs() values, so we need to restore the sign
            // from the original point
            coords = NSPoint::new(
                f64::copysign(backing_rect.size.width, point.x),
                f64::copysign(backing_rect.size.height, point.y),
            );
            mouse_buttons = decode_mouse_buttons(NSEvent::pressedMouseButtons(nsevent));
            modifiers = key_modifiers(nsevent.modifierFlags());
            screen_coords = NSEvent::mouseLocation(nsevent);
        }
        let event = MouseEvent {
            kind,
            coords: Point::new(coords.x as isize, coords.y as isize),
            screen_coords: cartesian_to_screen_point(screen_coords),
            mouse_buttons,
            modifiers,
        };

        if let Some(myself) = Self::get_this(this) {
            let mut inner = myself.inner.borrow_mut();
            inner.events.dispatch(WindowEvent::MouseEvent(event));
        }
    }

    extern "C" fn mouse_up(this: &mut Object, _sel: Sel, nsevent: id) {
        Self::mouse_common(this, nsevent, MouseEventKind::Release(MousePress::Left));
    }

    extern "C" fn mouse_down(this: &mut Object, _sel: Sel, nsevent: id) {
        Self::mouse_common(this, nsevent, MouseEventKind::Press(MousePress::Left));
    }
    extern "C" fn right_mouse_up(this: &mut Object, _sel: Sel, nsevent: id) {
        Self::mouse_common(this, nsevent, MouseEventKind::Release(MousePress::Right));
    }

    extern "C" fn other_mouse_up(this: &mut Object, _sel: Sel, nsevent: id) {
        // Safety: We know this is an button event
        unsafe {
            let button_number = NSEvent::buttonNumber(nsevent);
            // Button 2 is the middle mouse button (scroll wheel)
            // but is the dedicated middle mouse button on 4 button mouses
            if button_number == 2 {
                Self::mouse_common(this, nsevent, MouseEventKind::Release(MousePress::Middle));
            }
        }
    }

    extern "C" fn scroll_wheel(this: &mut Object, _sel: Sel, nsevent: id) {
        let precise = unsafe { nsevent.hasPreciseScrollingDeltas() } == YES;
        let scale = if precise {
            // Devices with precise deltas report number of pixels scrolled.
            // At this layer we don't know how many pixels comprise a cell
            // in the terminal widget, and our abstraction doesn't allow being
            // told what that amount should be, so we come up with a hard
            // coded factor based on the likely default font size and dpi
            // to make the scroll speed feel a bit better.
            15.0
        } else {
            // Whereas imprecise deltas report the number of lines scrolled,
            // so we want to report those lines here wholesale.
            1.0
        };
        let mut vert_delta = unsafe { nsevent.scrollingDeltaY() } / scale;
        let mut horz_delta = unsafe { nsevent.scrollingDeltaX() } / scale;

        if let Some(myself) = Self::get_this(this) {
            let mut inner = myself.inner.borrow_mut();

            let elapsed = inner.last_wheel.elapsed();

            // If it's been a while since the last wheel movement,
            // we want to clear out any accumulated fractional amount
            // and round this event up to 1 line so that we get an
            // immediate scroll on the first move.
            let stale = std::time::Duration::from_millis(250);
            if elapsed >= stale {
                if vert_delta != 0.0 && vert_delta.abs() < 1.0 {
                    vert_delta = round_away_from_zerof(vert_delta);
                }
                if horz_delta != 0.0 && horz_delta.abs() < 1.0 {
                    horz_delta = round_away_from_zerof(horz_delta);
                }
                inner.vscroll_remainder = 0.;
                inner.hscroll_remainder = 0.;
            }

            inner.last_wheel = Instant::now();

            // Reset remainder when changing scroll direction
            if vert_delta.signum() != inner.vscroll_remainder.signum() {
                inner.vscroll_remainder = 0.;
            }
            if horz_delta.signum() != inner.hscroll_remainder.signum() {
                inner.hscroll_remainder = 0.;
            }

            vert_delta += inner.vscroll_remainder;
            horz_delta += inner.hscroll_remainder;

            inner.vscroll_remainder = vert_delta.fract();
            inner.hscroll_remainder = horz_delta.fract();

            vert_delta = vert_delta.trunc();
            horz_delta = horz_delta.trunc();
        }

        if vert_delta.abs() < 1.0 && horz_delta.abs() < 1.0 {
            return;
        }

        let kind = if vert_delta.abs() > horz_delta.abs() {
            MouseEventKind::VertWheel(round_away_from_zero(vert_delta))
        } else {
            MouseEventKind::HorzWheel(round_away_from_zero(horz_delta))
        };
        Self::mouse_common(this, nsevent, kind);
    }

    extern "C" fn right_mouse_down(this: &mut Object, _sel: Sel, nsevent: id) {
        Self::mouse_common(this, nsevent, MouseEventKind::Press(MousePress::Right));
    }

    extern "C" fn other_mouse_down(this: &mut Object, _sel: Sel, nsevent: id) {
        // Safety: See `other_mouse_up`
        unsafe {
            let button_number = NSEvent::buttonNumber(nsevent);
            // See `other_mouse_up`
            if button_number == 2 {
                Self::mouse_common(this, nsevent, MouseEventKind::Press(MousePress::Middle));
            }
        }
    }

    extern "C" fn mouse_moved_or_dragged(this: &mut Object, _sel: Sel, nsevent: id) {
        Self::mouse_common(this, nsevent, MouseEventKind::Move);
    }

    extern "C" fn mouse_exited(this: &mut Object, _sel: Sel, _nsevent: id) {
        if let Some(myself) = Self::get_this(this) {
            myself
                .inner
                .borrow_mut()
                .events
                .dispatch(WindowEvent::MouseLeave);
        }
    }

    fn key_common(this: &mut Object, nsevent: id, key_is_down: bool) {
        let is_a_repeat = unsafe { nsevent.isARepeat() == YES };
        let chars = unsafe { nsstring_to_str(nsevent.characters()) };
        let unmod = unsafe { nsstring_to_str(nsevent.charactersIgnoringModifiers()) };
        let modifier_flags = unsafe { nsevent.modifierFlags() };
        let modifiers = key_modifiers(modifier_flags);
        let virtual_key = unsafe { nsevent.keyCode() };

        log::debug!(
            "key_common: chars=`{}` unmod=`{}` modifiers=`{:?}` virtual_key={:?} key_is_down:{}",
            chars.escape_debug(),
            unmod.escape_debug(),
            modifiers,
            virtual_key,
            key_is_down
        );

        // `Delete` on macos is really Backspace and emits BS.
        // `Fn-Delete` emits DEL.
        // Alt-Delete is mapped by the IME to be equivalent to Fn-Delete.
        // We want to emit Alt-BS in that situation.
        let (prefer_vkey, unmod) =
            if virtual_key == kVK_Delete && modifiers.contains(Modifiers::ALT) {
                (true, "\x08")
            } else if virtual_key == kVK_Tab {
                (true, "\t")
            } else if virtual_key == kVK_Delete {
                (true, "\x08")
            } else if virtual_key == kVK_ANSI_KeypadEnter {
                // https://github.com/wez/wezterm/issues/739
                // Keypad enter sends ctrl-c for some reason; explicitly
                // treat that as enter here.
                (true, "\r")
            } else {
                (false, unmod)
            };

        // Shift-Tab on macOS produces \x19 for some reason.
        // Rewrite it to something we understand.
        // <https://github.com/wez/wezterm/issues/1902>
        let chars = if virtual_key == kVK_Tab && modifiers.contains(Modifiers::SHIFT) {
            "\t"
        } else {
            chars
        };

        let phys_code = vkey_to_phys(virtual_key);
        let raw_key_handled = Handled::new();
        let raw_key_event = RawKeyEvent {
            key: if unmod.is_empty() {
                match phys_code {
                    Some(phys) => KeyCode::Physical(phys),
                    None => KeyCode::RawCode(virtual_key as _),
                }
            } else {
                KeyCode::composed(unmod)
            },
            phys_code,
            raw_code: virtual_key as _,
            modifiers,
            repeat_count: 1,
            key_is_down,
            handled: raw_key_handled.clone(),
        };
        if let Some(myself) = Self::get_this(this) {
            let mut inner = myself.inner.borrow_mut();
            inner
                .events
                .dispatch(WindowEvent::RawKeyEvent(raw_key_event.clone()));
        }

        if raw_key_handled.is_handled() {
            log::trace!("raw key was handled; not processing further");
            return;
        }

        let chars = if let Some(myself) = Self::get_this(this) {
            let mut inner = myself.inner.borrow_mut();

            if chars.is_empty() || inner.dead_pending.is_some() {
                // Dead key!
                if !key_is_down {
                    return;
                }

                match inner.translate_key_event(virtual_key, modifier_flags) {
                    Ok(TranslateStatus::Composing(composing)) => {
                        // Next key press in dead key sequence is pending.
                        inner.events.dispatch(WindowEvent::AdviseDeadKeyStatus(
                            DeadKeyStatus::Composing(composing),
                        ));

                        return;
                    }
                    Ok(TranslateStatus::Composed(translated)) => {
                        inner
                            .events
                            .dispatch(WindowEvent::AdviseDeadKeyStatus(DeadKeyStatus::None));
                        let event = KeyEvent {
                            key: KeyCode::composed(&translated),
                            modifiers: Modifiers::NONE,
                            repeat_count: 1,
                            key_is_down,
                            raw: None,
                        };
                        inner.events.dispatch(WindowEvent::KeyEvent(event));
                        return;
                    }
                    Ok(TranslateStatus::NotDead) => {
                        // Turned out that while it would have been a dead
                        // key combo, our send_composed_key_when_XXX settings
                        // said otherwise. Let's continue as if it was not
                        // a dead key.
                        unmod
                    }
                    Err(e) => {
                        log::error!("Failed to translate dead key: {}", e);
                        return;
                    }
                }
            } else {
                chars
            }
        } else {
            return;
        };

        let config_handle = config::configuration();
        let use_ime = config_handle.use_ime;
        let send_composed_key_when_left_alt_is_pressed =
            config_handle.send_composed_key_when_left_alt_is_pressed;
        let send_composed_key_when_right_alt_is_pressed =
            config_handle.send_composed_key_when_right_alt_is_pressed;

        // If unmod is empty it most likely means that the user has selected
        // an alternate keymap that has a chorded representation of eg: an ASCII
        // character.  One example of this is selecting a Norwegian keymap on
        // a US keyboard.  The `~` symbol is produced by pressing CTRL-].
        // That shows up here as unmod=`` with modifiers=CTRL.  In this situation
        // we want to cancel the modifiers out so that we just focus on
        // `chars` instead.
        let modifiers = if unmod.is_empty() {
            Modifiers::NONE
        } else {
            modifiers
        };

        let alt_mods = Modifiers::LEFT_ALT | Modifiers::RIGHT_ALT | Modifiers::ALT;
        let only_left_alt = (modifiers & alt_mods) == (Modifiers::LEFT_ALT | Modifiers::ALT);
        let only_right_alt = (modifiers & alt_mods) == (Modifiers::RIGHT_ALT | Modifiers::ALT);

        // Also respect `send_composed_key_when_(left|right)_alt_is_pressed` configs
        // when `use_ime` is true.
        let forward_to_ime = {
            if only_left_alt && !send_composed_key_when_left_alt_is_pressed {
                false
            } else if only_right_alt && !send_composed_key_when_right_alt_is_pressed {
                false
            } else {
                true
            }
        };

        if key_is_down && use_ime && forward_to_ime {
            if let Some(myself) = Self::get_this(this) {
                let mut inner = myself.inner.borrow_mut();
                inner.key_is_down.replace(key_is_down);
                inner.ime_state = ImeDisposition::None;
                inner.ime_text.clear();
            }

            unsafe {
                let array: id = msg_send![class!(NSArray), arrayWithObject: nsevent];
                let _: () = msg_send![this, interpretKeyEvents: array];

                if let Some(myself) = Self::get_this(this) {
                    let mut inner = myself.inner.borrow_mut();
                    log::trace!(
                        "IME state: {:?}, last_event: {:?}",
                        inner.ime_state,
                        inner.ime_last_event
                    );
                    match inner.ime_state {
                        ImeDisposition::Continue => {
                            // IME handled the event by generating NOOP;
                            // let's continue with our normal handling
                            // code below.
                            inner.ime_last_event.take();
                        }
                        ImeDisposition::Acted => {
                            // The key caused the IME to call one of our
                            // callbacks, which may have generated an event and
                            // stashed it into ime_last_event.
                            // If it didn't generate an event, then a composition
                            // is pending.
                            let status = if inner.ime_last_event.is_none() {
                                DeadKeyStatus::Composing(inner.ime_text.clone())
                            } else {
                                DeadKeyStatus::None
                            };
                            inner
                                .events
                                .dispatch(WindowEvent::AdviseDeadKeyStatus(status));
                            return;
                        }
                        ImeDisposition::None => {
                            // The IME clocked something in its state,
                            // but didn't call one of our callbacks.
                            // In theory, we should stop here, but the IME
                            // mysteriously swallows key repeats for certain
                            // keys (eg: `f`) but not others.
                            // To compensate for that, if the current event
                            // is a repeat, and the IME previously generated
                            // `Acted`, we will assume that we're safe to replay
                            // that last action.
                            if is_a_repeat {
                                if let Some(event) =
                                    inner.ime_last_event.as_ref().map(|e| e.clone())
                                {
                                    inner.events.dispatch(WindowEvent::KeyEvent(event));
                                    return;
                                }
                            }
                            let status = if inner.ime_text.is_empty() {
                                DeadKeyStatus::None
                            } else {
                                DeadKeyStatus::Composing(inner.ime_text.clone())
                            };
                            inner
                                .events
                                .dispatch(WindowEvent::AdviseDeadKeyStatus(status));
                            return;
                        }
                    }
                }
            }
        }

        fn key_string_to_key_code(s: &str) -> Option<KeyCode> {
            let mut char_iter = s.chars();
            if let Some(first_char) = char_iter.next() {
                if char_iter.next().is_none() {
                    // A single unicode char
                    Some(function_key_to_keycode(first_char))
                } else {
                    Some(KeyCode::Composed(s.to_owned()))
                }
            } else {
                None
            }
        }

        // When both shift and alt are pressed, macos appears to swap `chars` with `unmod`,
        // which isn't particularly helpful. eg: ALT+SHIFT+` produces chars='`' and unmod='~'
        // In this case, we take the key from unmod.
        // We leave `raw` set to None as we want to preserve the value of modifiers.
        // <https://github.com/wez/wezterm/issues/1706>.
        // We can't do this for every ALT+SHIFT combo, as the weird behavior doesn't
        // apply to eg: ALT+SHIFT+789 for Norwegian layouts
        // <https://github.com/wez/wezterm/issues/760>
        let swap_unmod_and_chars = (modifiers.contains(Modifiers::SHIFT | Modifiers::ALT)
            && virtual_key == kVK_ANSI_Grave)
            ||
            // <https://github.com/wez/wezterm/issues/1907>
            (modifiers.contains(Modifiers::SHIFT | Modifiers::CTRL)
                && virtual_key == kVK_ANSI_Slash);

        if let Some(key) = key_string_to_key_code(chars).or_else(|| key_string_to_key_code(unmod)) {
            let (key, raw_key) = if prefer_vkey {
                match phys_code {
                    Some(phys) => (phys.to_key_code(), None),
                    None => {
                        log::error!(
                            "prefer_vkey=true, but phys_code is None. {:?}",
                            raw_key_event
                        );
                        return;
                    }
                }
            } else if (only_left_alt && !send_composed_key_when_left_alt_is_pressed)
                || (only_right_alt && !send_composed_key_when_right_alt_is_pressed)
            {
                // Take the unmodified key only!
                match key_string_to_key_code(unmod) {
                    Some(key) => (key, None),
                    None => return,
                }
            } else if chars.is_empty() || chars == unmod {
                (key, None)
            } else if swap_unmod_and_chars {
                match key_string_to_key_code(unmod) {
                    Some(key) => (key, None),
                    None => return,
                }
            } else {
                let raw = key_string_to_key_code(unmod);
                match (&key, &raw) {
                    // Avoid eg: \x01 when we can use CTRL-A.
                    // This also helps to keep the correct sequence for backspace/delete.
                    // But take care: on German layouts CTRL-Backslash has unmod="/"
                    // but chars="\x1c"; we only want to do this transformation when
                    // chars and unmod have that base ASCII relationship.
                    // <https://github.com/wez/wezterm/issues/1891>
                    (KeyCode::Char(c), Some(KeyCode::Char(raw)))
                        if is_ascii_control(*c) == Some(raw.to_ascii_lowercase()) =>
                    {
                        (KeyCode::Char(*raw), None)
                    }
                    _ => (key, raw),
                }
            };

            let modifiers = if raw_key.is_some() {
                Modifiers::NONE
            } else {
                modifiers
            };

            let event = KeyEvent {
                key,
                modifiers,
                repeat_count: 1,
                key_is_down,
                raw: Some(raw_key_event),
            }
            .normalize_shift();

            log::debug!(
                "key_common {:?} (chars={:?} unmod={:?} modifiers={:?})",
                event,
                chars,
                unmod,
                modifiers
            );

            if let Some(myself) = Self::get_this(this) {
                let mut inner = myself.inner.borrow_mut();
                inner.ime_last_event.take();
                inner.events.dispatch(WindowEvent::KeyEvent(event));
            }
        }
    }

    extern "C" fn perform_key_equivalent(this: &mut Object, _sel: Sel, nsevent: id) -> BOOL {
        let chars = unsafe { nsstring_to_str(nsevent.characters()) };
        let modifier_flags = unsafe { nsevent.modifierFlags() };
        let modifiers = key_modifiers(modifier_flags);

        log::trace!(
            "perform_key_equivalent: chars=`{}` modifiers=`{:?}`",
            chars.escape_debug(),
            modifiers,
        );

        if (chars == "." && modifiers == Modifiers::SUPER)
            || (chars == "\u{1b}" && modifiers == Modifiers::CTRL)
            || (chars == "\t" && modifiers == Modifiers::CTRL)
            || (chars == "\x19"/* Shift-Tab: See issue #1902 */)
        {
            // Synthesize a key down event for this, because macOS will
            // not do that, even though we tell it that we handled this event.
            // <https://github.com/wez/wezterm/issues/1867>
            Self::key_common(this, nsevent, true);

            // Prevent macOS from calling doCommandBySelector(cancel:)
            YES
        } else {
            // Allow macOS to process built-in shortcuts like CMD-`
            // to cycle though windows
            NO
        }
    }

    extern "C" fn key_down(this: &mut Object, _sel: Sel, nsevent: id) {
        Self::key_common(this, nsevent, true);
    }

    extern "C" fn key_up(this: &mut Object, _sel: Sel, nsevent: id) {
        Self::key_common(this, nsevent, false);
    }

    extern "C" fn did_change_screen(this: &mut Object, _sel: Sel, _notification: id) {
        log::trace!("did_change_screen");
        if let Some(this) = Self::get_this(this) {
            // Just set a flag; we don't want to react immediately
            // as this even fires as part of a live move and the
            // resize flow may try to re-position the window to
            // the wrong place.
            this.inner.borrow_mut().screen_changed = true;
        }
    }

    extern "C" fn will_start_live_resize(this: &mut Object, _sel: Sel, _notification: id) {
        if let Some(this) = Self::get_this(this) {
            let mut inner = this.inner.borrow_mut();
            inner.live_resizing = true;
        }
    }

    extern "C" fn did_end_live_resize(this: &mut Object, _sel: Sel, _notification: id) {
        if let Some(this) = Self::get_this(this) {
            let mut inner = this.inner.borrow_mut();
            inner.live_resizing = false;
        }
    }

    extern "C" fn did_resize(this: &mut Object, _sel: Sel, _notification: id) {
        if let Some(this) = Self::get_this(this) {
            let inner = this.inner.borrow_mut();

            if let Some(gl_context_pair) = inner.gl_context_pair.as_ref() {
                gl_context_pair.backend.update();
            }
        }

        let frame = unsafe { NSView::frame(this as *mut _) };
        let backing_frame = unsafe { NSView::convertRectToBacking(this as *mut _, frame) };
        let width = backing_frame.size.width;
        let height = backing_frame.size.height;
        if let Some(this) = Self::get_this(this) {
            let mut inner = this.inner.borrow_mut();

            // This is a little gross; ideally we'd call
            // WindowInner:is_fullscreen to determine this, but
            // we can't get a mutable reference to it from here
            // as we can be called in a context where something
            // higher up the callstack already has a mutable
            // reference and we'd panic.
            let is_full_screen = inner.fullscreen.is_some()
                || inner.window.as_ref().map_or(false, |window| {
                    let window = window.load();
                    let style_mask = unsafe { NSWindow::styleMask(*window) };
                    style_mask.contains(NSWindowStyleMask::NSFullScreenWindowMask)
                });

            let live_resizing = inner.live_resizing;

            inner.events.dispatch(WindowEvent::Resized {
                dimensions: Dimensions {
                    pixel_width: width as usize,
                    pixel_height: height as usize,
                    dpi: (crate::DEFAULT_DPI * (backing_frame.size.width / frame.size.width))
                        as usize,
                },
                window_state: if is_full_screen {
                    WindowState::FULL_SCREEN
                } else {
                    WindowState::default()
                },
                live_resizing,
            });
        }
    }

    extern "C" fn draw_rect(view: &mut Object, sel: Sel, _dirty_rect: NSRect) {
        if let Some(this) = Self::get_this(view) {
            let mut inner = this.inner.borrow_mut();

            if inner.screen_changed {
                // If the screen resolution changed (which can also
                // happen if the window was dragged to another monitor
                // with different dpi), then we treat this as a resize
                // event that will in turn trigger an invalidation
                // and a repaint.
                inner.screen_changed = false;
                drop(inner);
                Self::did_resize(view, sel, nil);
                return;
            }

            if inner.paint_throttled {
                inner.invalidated = true;
            } else {
                inner.events.dispatch(WindowEvent::NeedRepaint);
                inner.invalidated = false;
                inner.paint_throttled = true;

                let window_id = inner.window_id;
                let max_fps = inner.config.max_fps;
                promise::spawn::spawn(async move {
                    async_io::Timer::after(std::time::Duration::from_millis(1000 / max_fps as u64))
                        .await;
                    Connection::with_window_inner(window_id, move |inner| {
                        if let Some(window_view) = WindowView::get_this(unsafe { &**inner.view }) {
                            let mut state = window_view.inner.borrow_mut();
                            state.paint_throttled = false;
                            if state.invalidated {
                                unsafe {
                                    let () = msg_send![*inner.view, setNeedsDisplay: YES];
                                }
                            }
                        }
                        Ok(())
                    });
                })
                .detach();
            }
        }
    }

    extern "C" fn dragging_entered(this: &mut Object, _: Sel, sender: id) -> BOOL {
        if let Some(this) = Self::get_this(this) {
            let mut inner = this.inner.borrow_mut();

            let pb: id = unsafe { msg_send![sender, draggingPasteboard] };
            let filenames =
                unsafe { NSPasteboard::propertyListForType(pb, appkit::NSFilenamesPboardType) };
            let paths = unsafe { filenames.iter() }
                .map(|file| unsafe {
                    let path = nsstring_to_str(file);
                    PathBuf::from(path)
                })
                .collect::<Vec<_>>();
            inner.events.dispatch(WindowEvent::DraggedFile(paths));
        }
        YES
    }

    extern "C" fn perform_drag_operation(this: &mut Object, _: Sel, sender: id) -> BOOL {
        if let Some(this) = Self::get_this(this) {
            let mut inner = this.inner.borrow_mut();

            let pb: id = unsafe { msg_send![sender, draggingPasteboard] };
            let filenames =
                unsafe { NSPasteboard::propertyListForType(pb, appkit::NSFilenamesPboardType) };
            let paths = unsafe { filenames.iter() }
                .map(|file| unsafe {
                    let path = nsstring_to_str(file);
                    PathBuf::from(path)
                })
                .collect::<Vec<_>>();
            inner.events.dispatch(WindowEvent::DroppedFile(paths));
        }
        YES
    }

    fn get_this(this: &Object) -> Option<&mut Self> {
        unsafe {
            let myself: *mut c_void = *this.get_ivar(VIEW_CLS_NAME);
            if myself.is_null() {
                None
            } else {
                Some(&mut *(myself as *mut Self))
            }
        }
    }

    fn alloc(inner: &Rc<RefCell<Inner>>) -> anyhow::Result<StrongPtr> {
        let cls = Self::get_class();

        let view_id: StrongPtr = unsafe { StrongPtr::new(msg_send![cls, new]) };

        inner.borrow_mut().view_id.replace(view_id.weak());

        let view = Box::into_raw(Box::new(Self {
            inner: Rc::clone(&inner),
        }));

        unsafe {
            (**view_id).set_ivar(VIEW_CLS_NAME, view as *mut c_void);
        }

        Ok(view_id)
    }

    fn get_class() -> &'static Class {
        Class::get(VIEW_CLS_NAME).unwrap_or_else(Self::define_class)
    }

    fn define_class() -> &'static Class {
        let mut cls = ClassDecl::new(VIEW_CLS_NAME, class!(NSView))
            .expect("Unable to register WindowView class");

        cls.add_ivar::<*mut c_void>(VIEW_CLS_NAME);
        cls.add_protocol(
            Protocol::get("NSTextInputClient").expect("failed to get NSTextInputClient protocol"),
        );

        unsafe {
            cls.add_method(
                sel!(dealloc),
                WindowView::dealloc as extern "C" fn(&mut Object, Sel),
            );

            cls.add_method(
                sel!(windowWillClose:),
                Self::window_will_close as extern "C" fn(&mut Object, Sel, id),
            );

            cls.add_method(
                sel!(windowShouldClose:),
                Self::window_should_close as extern "C" fn(&mut Object, Sel, id) -> BOOL,
            );

            cls.add_method(
                sel!(drawRect:),
                Self::draw_rect as extern "C" fn(&mut Object, Sel, NSRect),
            );

            cls.add_method(
                sel!(isFlipped),
                Self::is_flipped as extern "C" fn(&Object, Sel) -> BOOL,
            );

            cls.add_method(
                sel!(isOpaque),
                Self::is_opaque as extern "C" fn(&Object, Sel) -> BOOL,
            );

            cls.add_method(
                sel!(allowsAutomaticWindowTabbing),
                Self::allow_automatic_tabbing as extern "C" fn(&Object, Sel) -> BOOL,
            );

            cls.add_method(
                sel!(windowWillStartLiveResize:),
                Self::will_start_live_resize as extern "C" fn(&mut Object, Sel, id),
            );
            cls.add_method(
                sel!(windowDidEndLiveResize:),
                Self::did_end_live_resize as extern "C" fn(&mut Object, Sel, id),
            );
            cls.add_method(
                sel!(windowDidResize:),
                Self::did_resize as extern "C" fn(&mut Object, Sel, id),
            );
            cls.add_method(
                sel!(windowDidChangeScreen:),
                Self::did_change_screen as extern "C" fn(&mut Object, Sel, id),
            );

            cls.add_method(
                sel!(windowDidBecomeKey:),
                Self::did_become_key as extern "C" fn(&mut Object, Sel, id),
            );
            cls.add_method(
                sel!(windowDidResignKey:),
                Self::did_resign_key as extern "C" fn(&mut Object, Sel, id),
            );

            cls.add_method(
                sel!(mouseMoved:),
                Self::mouse_moved_or_dragged as extern "C" fn(&mut Object, Sel, id),
            );
            cls.add_method(
                sel!(mouseDragged:),
                Self::mouse_moved_or_dragged as extern "C" fn(&mut Object, Sel, id),
            );
            cls.add_method(
                sel!(rightMouseDragged:),
                Self::mouse_moved_or_dragged as extern "C" fn(&mut Object, Sel, id),
            );
            cls.add_method(
                sel!(mouseDown:),
                Self::mouse_down as extern "C" fn(&mut Object, Sel, id),
            );
            cls.add_method(
                sel!(mouseUp:),
                Self::mouse_up as extern "C" fn(&mut Object, Sel, id),
            );
            cls.add_method(
                sel!(rightMouseDown:),
                Self::right_mouse_down as extern "C" fn(&mut Object, Sel, id),
            );
            cls.add_method(
                sel!(rightMouseUp:),
                Self::right_mouse_up as extern "C" fn(&mut Object, Sel, id),
            );
            cls.add_method(
                sel!(otherMouseDown:),
                Self::other_mouse_down as extern "C" fn(&mut Object, Sel, id),
            );
            cls.add_method(
                sel!(otherMouseUp:),
                Self::other_mouse_up as extern "C" fn(&mut Object, Sel, id),
            );
            cls.add_method(
                sel!(scrollWheel:),
                Self::scroll_wheel as extern "C" fn(&mut Object, Sel, id),
            );
            cls.add_method(
                sel!(mouseExited:),
                Self::mouse_exited as extern "C" fn(&mut Object, Sel, id),
            );

            cls.add_method(
                sel!(keyDown:),
                Self::key_down as extern "C" fn(&mut Object, Sel, id),
            );
            cls.add_method(
                sel!(keyUp:),
                Self::key_up as extern "C" fn(&mut Object, Sel, id),
            );

            cls.add_method(
                sel!(performKeyEquivalent:),
                Self::perform_key_equivalent as extern "C" fn(&mut Object, Sel, id) -> BOOL,
            );

            cls.add_method(
                sel!(acceptsFirstResponder),
                Self::accepts_first_responder as extern "C" fn(&mut Object, Sel) -> BOOL,
            );

            cls.add_method(
                sel!(acceptsFirstMouse:),
                Self::accepts_first_mouse as extern "C" fn(&mut Object, Sel, id) -> BOOL,
            );

            cls.add_method(
                sel!(viewDidChangeEffectiveAppearance),
                Self::view_did_change_effective_appearance as extern "C" fn(&mut Object, Sel),
            );

            cls.add_method(
                sel!(updateTrackingAreas),
                Self::update_tracking_areas as extern "C" fn(&mut Object, Sel),
            );

            // NSTextInputClient

            cls.add_method(
                sel!(hasMarkedText),
                Self::has_marked_text as extern "C" fn(&mut Object, Sel) -> BOOL,
            );
            cls.add_method(
                sel!(markedRange),
                Self::marked_range as extern "C" fn(&mut Object, Sel) -> NSRange,
            );
            cls.add_method(
                sel!(selectedRange),
                Self::selected_range as extern "C" fn(&mut Object, Sel) -> NSRange,
            );
            cls.add_method(
                sel!(setMarkedText:selectedRange:replacementRange:),
                Self::set_marked_text_selected_range_replacement_range
                    as extern "C" fn(&mut Object, Sel, id, NSRange, NSRange),
            );
            cls.add_method(
                sel!(unmarkText),
                Self::unmark_text as extern "C" fn(&mut Object, Sel),
            );
            cls.add_method(
                sel!(validAttributesForMarkedText),
                Self::valid_attributes_for_marked_text as extern "C" fn(&mut Object, Sel) -> id,
            );
            cls.add_method(
                sel!(doCommandBySelector:),
                Self::do_command_by_selector as extern "C" fn(&mut Object, Sel, Sel),
            );

            cls.add_method(
                sel!( attributedSubstringForProposedRange:actualRange:),
                Self::attributed_substring_for_proposed_range
                    as extern "C" fn(&mut Object, Sel, NSRange, NSRangePointer) -> id,
            );
            cls.add_method(
                sel!(insertText:replacementRange:),
                Self::insert_text_replacement_range as extern "C" fn(&mut Object, Sel, id, NSRange),
            );

            cls.add_method(
                sel!(characterIndexForPoint:),
                Self::character_index_for_point
                    as extern "C" fn(&mut Object, Sel, NSPoint) -> NSUInteger,
            );
            cls.add_method(
                sel!(firstRectForCharacterRange:actualRange:),
                Self::first_rect_for_character_range
                    as extern "C" fn(&mut Object, Sel, NSRange, NSRangePointer) -> NSRect,
            );
            cls.add_method(
                sel!(draggingEntered:),
                Self::dragging_entered as extern "C" fn(&mut Object, Sel, id) -> BOOL,
            );
            cls.add_method(
                sel!(performDragOperation:),
                Self::perform_drag_operation as extern "C" fn(&mut Object, Sel, id) -> BOOL,
            );
        }

        cls.register()
    }
}
