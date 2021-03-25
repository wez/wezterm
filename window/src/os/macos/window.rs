// let () = msg_send! is a common pattern for objc
#![allow(clippy::let_unit_value)]

use super::{nsstring, nsstring_to_str};
use crate::connection::ConnectionOps;
use crate::{
    Clipboard, Connection, Dimensions, GpuContext, KeyCode, KeyEvent, Modifiers, MouseButtons,
    MouseCursor, MouseEvent, MouseEventKind, MousePress, Point, Rect, ScreenPoint, Size,
    WindowCallbacks, WindowDecorations, WindowOps, WindowOpsMut,
};
use anyhow::Context;
use anyhow::{anyhow, bail};
use cocoa::appkit::{
    self, NSApplication, NSApplicationActivateIgnoringOtherApps, NSApplicationPresentationOptions,
    NSBackingStoreBuffered, NSEvent, NSEventModifierFlags, NSRunningApplication, NSScreen, NSView,
    NSViewHeightSizable, NSViewWidthSizable, NSWindow, NSWindowStyleMask,
};
use cocoa::base::*;
use cocoa::foundation::{NSArray, NSNotFound, NSPoint, NSRect, NSSize, NSUInteger};
use config::ConfigHandle;
use core_foundation::base::{CFTypeID, TCFType};
use core_foundation::data::{CFData, CFDataGetBytePtr, CFDataRef};
use core_foundation::string::{CFStringRef, UniChar};
use core_foundation::{declare_TCFType, impl_TCFType};
use core_graphics::base::CGFloat;
use objc::declare::ClassDecl;
use objc::rc::{StrongPtr, WeakPtr};
use objc::runtime::{Class, Object, Protocol, Sel};
use objc::*;
use promise::Future;
use raw_window_handle::macos::MacOSHandle;
use raw_window_handle::{HasRawWindowHandle, RawWindowHandle};
use std::any::Any;
use std::cell::RefCell;
use std::ffi::c_void;
use std::rc::Rc;
use std::time::Instant;

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

pub(crate) struct WindowInner {
    window_id: usize,
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

#[derive(Debug, Clone)]
pub struct Window(usize);

impl Window {
    pub async fn new_window(
        _class_name: &str,
        name: &str,
        width: usize,
        height: usize,
        callbacks: Box<dyn WindowCallbacks>,
        config: Option<&ConfigHandle>,
    ) -> anyhow::Result<Window> {
        let config = match config {
            Some(c) => c.clone(),
            None => config::configuration(),
        };

        unsafe {
            let style_mask = decoration_to_mask(config.window_decorations);
            let rect = NSRect::new(
                NSPoint::new(0., 0.),
                NSSize::new(width as f64, height as f64),
            );

            let conn = Connection::get().expect("Connection::init has not been called");

            let window_id = conn.next_window_id();

            let inner = Rc::new(RefCell::new(Inner {
                callbacks,
                view_id: None,
                window: None,
                window_id,
                screen_changed: false,
                gpu_context: None,
                text_cursor_position: Rect::new(Point::new(0, 0), Size::new(0, 0)),
                hscroll_remainder: 0.,
                vscroll_remainder: 0.,
                last_wheel: Instant::now(),
                key_is_down: None,
                dead_pending: None,
                fullscreen: None,
                config: config.clone(),
            }));

            let window: id = msg_send![get_window_class(), alloc];
            let window = StrongPtr::new(NSWindow::initWithContentRect_styleMask_backing_defer_(
                window,
                rect,
                style_mask,
                NSBackingStoreBuffered,
                NO,
            ));

            // Prevent Cocoa native tabs from being used
            let _: () = msg_send![*window, setTabbingMode:2 /* NSWindowTabbingModeDisallowed */];

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
            LAST_POSITION.with(|pos| {
                let next_pos = if let Some(last_pos) = pos.borrow_mut().take() {
                    window.cascadeTopLeftFromPoint_(last_pos)
                } else {
                    window.center();
                    window.cascadeTopLeftFromPoint_(NSPoint::new(0.0, 0.0))
                };
                pos.borrow_mut().replace(next_pos);
            });

            window.setTitle_(*nsstring(&name));
            window.setAcceptsMouseMovedEvents_(YES);

            let view = WindowView::alloc(&inner)?;
            view.initWithFrame_(rect);
            view.setAutoresizingMask_(NSViewHeightSizable | NSViewWidthSizable);

            view.setWantsBestResolutionOpenGLSurface_(YES);

            window.setContentView_(*view);
            window.setDelegate_(*view);

            view.setWantsLayer(YES);
            let () = msg_send![*view, setLayerContentsRedrawPolicy:2 /* NSViewLayerContentsRedrawDuringViewResize */];

            let frame = NSView::frame(*view);
            let backing_frame = NSView::convertRectToBacking(*view, frame);
            let width = backing_frame.size.width;
            let height = backing_frame.size.height;

            let weak_window = window.weak();
            let window_inner = Rc::new(RefCell::new(WindowInner {
                window_id,
                window,
                view,
                config: config.clone(),
            }));
            inner.borrow_mut().window.replace(weak_window);
            conn.windows
                .borrow_mut()
                .insert(window_id, Rc::clone(&window_inner));

            let window = Window(window_id);
            window.config_did_change(&config);

            inner
                .borrow_mut()
                .enable_wgpu(width as u32, height as u32)
                .await?;

            let view = *inner.borrow().view_id.as_ref().unwrap().load();
            // Force an initial paint
            let () = msg_send![view, setNeedsDisplay: YES];
            // Allow transparency, as the default for Metal is opaque
            let layer: id = msg_send![view, layer];
            let () = msg_send![layer, setOpaque: NO];

            // Synthesize a resize event immediately; this allows
            // the embedding application an opportunity to discover
            // the dpi and adjust for display scaling
            inner.borrow_mut().callbacks.resize(
                Dimensions {
                    pixel_width: width as usize,
                    pixel_height: height as usize,
                    dpi: (crate::DEFAULT_DPI * (backing_frame.size.width / frame.size.width))
                        as usize,
                },
                false,
            );

            Ok(window)
        }
    }
}

impl WindowOps for Window {
    fn close(&self) -> Future<()> {
        Connection::with_window_inner(self.0, |inner| {
            inner.close();
            Ok(())
        })
    }

    fn hide(&self) -> Future<()> {
        Connection::with_window_inner(self.0, |inner| {
            inner.hide();
            Ok(())
        })
    }

    fn show(&self) -> Future<()> {
        Connection::with_window_inner(self.0, |inner| {
            inner.show();
            Ok(())
        })
    }

    fn set_cursor(&self, cursor: Option<MouseCursor>) -> Future<()> {
        Connection::with_window_inner(self.0, move |inner| {
            let _ = inner.set_cursor(cursor);
            Ok(())
        })
    }

    fn invalidate(&self) -> Future<()> {
        Connection::with_window_inner(self.0, |inner| {
            inner.invalidate();
            Ok(())
        })
    }

    fn set_title(&self, title: &str) -> Future<()> {
        let title = title.to_owned();
        Connection::with_window_inner(self.0, move |inner| {
            inner.set_title(&title);
            Ok(())
        })
    }

    fn set_inner_size(&self, width: usize, height: usize) -> Future<()> {
        Connection::with_window_inner(self.0, move |inner| {
            inner.set_inner_size(width, height);
            Ok(())
        })
    }

    fn set_window_position(&self, coords: ScreenPoint) -> Future<()> {
        Connection::with_window_inner(self.0, move |inner| {
            inner.set_window_position(coords);
            Ok(())
        })
    }

    fn set_text_cursor_position(&self, cursor: Rect) -> Future<()> {
        Connection::with_window_inner(self.0, move |inner| {
            inner.set_text_cursor_position(cursor);
            Ok(())
        })
    }

    fn apply<R, F: Send + 'static + FnMut(&mut dyn Any, &dyn WindowOps) -> anyhow::Result<R>>(
        &self,
        mut func: F,
    ) -> promise::Future<R>
    where
        Self: Sized,
        R: Send + 'static,
    {
        Connection::with_window_inner(self.0, move |inner| {
            let window = Window(inner.window_id);

            if let Some(window_view) = WindowView::get_this(unsafe { &**inner.view }) {
                func(window_view.inner.borrow_mut().callbacks.as_any(), &window)
            } else {
                bail!("apply: window is invalid");
            }
        })
    }

    fn get_clipboard(&self, _clipboard: Clipboard) -> Future<String> {
        use clipboard::ClipboardProvider;
        Future::result(
            clipboard::ClipboardContext::new()
                .and_then(|mut ctx| ctx.get_contents())
                .map_err(|e| anyhow!("Failed to get clipboard:{}", e)),
        )
    }

    fn set_clipboard(&self, _clipboard: Clipboard, text: String) -> Future<()> {
        use clipboard::ClipboardProvider;
        Future::result(
            clipboard::ClipboardContext::new()
                .and_then(|mut ctx| ctx.set_contents(text))
                .map_err(|e| anyhow!("Failed to set clipboard:{}", e)),
        )
    }

    fn toggle_fullscreen(&self) -> Future<()> {
        Connection::with_window_inner(self.0, move |inner| {
            inner.toggle_fullscreen();
            Ok(())
        })
    }

    fn config_did_change(&self, config: &ConfigHandle) -> Future<()> {
        let config = config.clone();
        Connection::with_window_inner(self.0, move |inner| {
            inner.config_did_change(&config);
            Ok(())
        })
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
            let mask = decoration_to_mask(self.config.window_decorations);
            unsafe {
                self.window.setStyleMask_(mask);
                /*
                NSWindow::setMovableByWindowBackground_(
                    *self.window,
                    if mask == NSWindowStyleMask::NSResizableWindowMask {
                        YES
                    } else {
                        NO
                    },
                );
                */
            }
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
                    self.window
                        .setStyleMask_(decoration_to_mask(self.config.window_decorations));
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

impl WindowOpsMut for WindowInner {
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
                let instance: id = match cursor {
                    MouseCursor::Arrow => msg_send![ns_cursor_cls, arrowCursor],
                    MouseCursor::Text => msg_send![ns_cursor_cls, IBeamCursor],
                    MouseCursor::Hand => msg_send![ns_cursor_cls, pointingHandCursor],
                    MouseCursor::SizeUpDown => msg_send![ns_cursor_cls, resizeUpDownCursor],
                    MouseCursor::SizeLeftRight => msg_send![ns_cursor_cls, resizeLeftRightCursor],
                };
                let () = msg_send![instance, set];
            }
        }
    }

    fn invalidate(&mut self) {
        unsafe {
            let () = msg_send![*self.view, setNeedsDisplay: YES];
        }
    }
    fn set_title(&mut self, title: &str) {
        let title = nsstring(title);
        unsafe {
            NSWindow::setTitle_(*self.window, *title);
        }
    }

    fn set_inner_size(&mut self, width: usize, height: usize) {
        unsafe {
            let frame = NSView::frame(*self.view as *mut _);
            let backing_frame = NSView::convertRectToBacking(*self.view as *mut _, frame);
            let scale = backing_frame.size.width / frame.size.width;

            NSWindow::setContentSize_(
                *self.window,
                NSSize::new(width as f64 / scale, height as f64 / scale),
            );
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

    fn config_did_change(&mut self, config: &ConfigHandle) {
        self.config = config.clone();
        if let Some(window_view) = WindowView::get_this(unsafe { &**self.view }) {
            window_view.inner.borrow_mut().config = config.clone();
        }
        self.update_window_shadow();
        self.apply_decorations();
    }
}

fn decoration_to_mask(decorations: WindowDecorations) -> NSWindowStyleMask {
    if decorations == WindowDecorations::TITLE | WindowDecorations::RESIZE {
        NSWindowStyleMask::NSTitledWindowMask
            | NSWindowStyleMask::NSClosableWindowMask
            | NSWindowStyleMask::NSMiniaturizableWindowMask
            | NSWindowStyleMask::NSResizableWindowMask
    } else if decorations == WindowDecorations::RESIZE {
        NSWindowStyleMask::NSResizableWindowMask
    } else if decorations == WindowDecorations::NONE {
        NSWindowStyleMask::NSBorderlessWindowMask
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

struct Inner {
    callbacks: Box<dyn WindowCallbacks>,
    view_id: Option<WeakPtr>,
    window: Option<WeakPtr>,
    window_id: usize,
    screen_changed: bool,
    gpu_context: Option<Rc<RefCell<GpuContext>>>,
    text_cursor_position: Rect,
    hscroll_remainder: f64,
    vscroll_remainder: f64,
    last_wheel: Instant,
    /// We use this to avoid double-emitting events when
    /// procesing key-up events.
    key_is_down: Option<bool>,

    /// First in a dead-key sequence
    dead_pending: Option<(u16, u32)>,

    /// When using simple fullscreen mode, this tracks
    /// the window dimensions that need to be restored
    fullscreen: Option<NSRect>,

    config: ConfigHandle,
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

unsafe impl HasRawWindowHandle for Inner {
    fn raw_window_handle(&self) -> RawWindowHandle {
        let ns_window = self.window.as_ref().unwrap().load();
        let ns_view = self.view_id.as_ref().unwrap().load();
        RawWindowHandle::MacOS(MacOSHandle {
            ns_window: *ns_window as *mut _,
            ns_view: *ns_view as *mut _,
            ..MacOSHandle::empty()
        })
    }
}

impl Inner {
    async fn enable_wgpu(&mut self, width: u32, height: u32) -> anyhow::Result<()> {
        let window = Window(self.window_id);

        let instance = wgpu::Instance::new(wgpu::BackendBit::PRIMARY);

        let surface = unsafe { instance.create_surface(self) };
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: Some(&surface),
            })
            .await
            .ok_or_else(|| anyhow::anyhow!("No suitable GPU adapters found on the system!"))?;

        let adapter_info = adapter.get_info();
        log::info!("wgpu adapter: {:?}", adapter_info);

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    features: wgpu::Features::empty(),
                    limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .context("Unable to find a suitable GPU adapter!")?;

        log::info!("wgpu device features: {:?}", device.features());
        log::info!("wgpu device limits: {:?}", device.limits());

        let sc_desc = wgpu::SwapChainDescriptor {
            usage: wgpu::TextureUsage::RENDER_ATTACHMENT,
            format: adapter.get_swap_chain_preferred_format(&surface),
            width,
            height,
            present_mode: wgpu::PresentMode::Mailbox,
        };
        let swap_chain = device.create_swap_chain(&surface, &sc_desc);

        let context = GpuContext {
            swap_chain,
            sc_desc,
            adapter,
            device,
            queue,
            surface,
        };

        self.gpu_context.replace(Rc::new(RefCell::new(context)));
        self.callbacks.created(
            &window,
            &mut *self.gpu_context.as_ref().unwrap().borrow_mut(),
        )?;

        Ok(())
    }

    /// <https://stackoverflow.com/a/22677690>
    /// <https://stackoverflow.com/a/12548163>
    /// <https://stackoverflow.com/a/8263841>
    /// <https://developer.apple.com/documentation/coreservices/1390584-uckeytranslate?language=objc>
    fn translate_key_event(
        &mut self,
        virtual_key_code: u16,
        modifier_flags: NSEventModifierFlags,
    ) -> Option<Result<String, std::string::FromUtf16Error>> {
        let kbd =
            unsafe { InputSource::wrap_under_create_rule(TISCopyCurrentKeyboardInputSource()) };

        let layout_data = unsafe {
            CFData::wrap_under_get_rule(TISGetInputSourceProperty(
                kbd.as_concrete_TypeRef(),
                kTISPropertyUnicodeKeyLayoutData,
            ))
        };

        let layout_data = unsafe {
            CFDataGetBytePtr(layout_data.as_concrete_TypeRef()) as *const UCKeyboardLayout
        };

        let modifier_key_state: u32 = (modifier_flags.bits() >> 16) as u32 & 0xFF;

        let kbd_type = unsafe { LMGetKbdType() } as _;

        let mut unicode_buffer = [0u16; 8];
        let mut length = 0;

        let mut dead_state = 0;

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

        if let Some((code, flags)) = self.dead_pending.take() {
            unsafe {
                UCKeyTranslate(
                    layout_data,
                    code,
                    kUCKeyActionDown,
                    flags,
                    kbd_type,
                    0,
                    &mut dead_state,
                    unicode_buffer.len() as _,
                    &mut length,
                    unicode_buffer.as_mut_ptr(),
                );
            }
        } else if use_dead_keys {
            self.dead_pending
                .replace((virtual_key_code, modifier_key_state));
            return None;
        }
        length = 0;
        unsafe {
            UCKeyTranslate(
                layout_data,
                virtual_key_code,
                kUCKeyActionDisplay,
                /*
                if key_is_down {
                kUCKeyActionDown
                } else {
                    kUCKeyActionUp
                },
                */
                modifier_key_state,
                kbd_type,
                0,
                &mut dead_state,
                unicode_buffer.len() as _,
                &mut length,
                unicode_buffer.as_mut_ptr(),
            );
        };

        if !use_dead_keys {
            length = 0;
            // Ignore dead key sequences; synthesize a SPACE press to
            // elicit the underlying key code
            unsafe {
                UCKeyTranslate(
                    layout_data,
                    super::keycodes::kVK_Space,
                    kUCKeyActionDown,
                    0,
                    kbd_type,
                    0,
                    &mut dead_state,
                    unicode_buffer.len() as _,
                    &mut length,
                    unicode_buffer.as_mut_ptr(),
                );
            }
        }

        Some(String::from_utf16(unsafe {
            std::slice::from_raw_parts(unicode_buffer.as_mut_ptr(), length as _)
        }))
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
        let mut modifiers = Modifiers::default();
        let key = match selector.as_ref() {
            "deleteBackward:" => KeyCode::Char('\x08'),
            "deleteForward:" => KeyCode::Char('\x7f'),
            "cancel:" => {
                // FIXME: this isn't scalable to various keys
                // and we lose eg: SHIFT if that is also pressed at the same time
                modifiers = Modifiers::CTRL;
                KeyCode::Char('\x1b')
            }
            "cancelOperation:" => KeyCode::Char('\x1b'),
            "insertNewline:" => KeyCode::Char('\r'),
            "insertTab:" => KeyCode::Char('\t'),
            "moveLeft:" => KeyCode::LeftArrow,
            "moveRight:" => KeyCode::RightArrow,
            "moveUp:" => KeyCode::UpArrow,
            "moveDown:" => KeyCode::DownArrow,
            "scrollToBeginningOfDocument:" => KeyCode::Home,
            "scrollToEndOfDocument:" => KeyCode::End,
            "scrollPageUp:" => KeyCode::PageUp,
            "scrollPageDown:" => KeyCode::PageDown,
            _ => {
                eprintln!("UNHANDLED: do_command_by_selector: {:?}", selector);
                return;
            }
        };

        let event = KeyEvent {
            key,
            raw_key: None,
            modifiers,
            raw_modifiers: Modifiers::NONE,
            raw_code: None,
            repeat_count: 1,
            key_is_down: true,
        }
        .normalize_shift();

        if let Some(myself) = Self::get_this(this) {
            let mut inner = myself.inner.borrow_mut();
            let window = Window(inner.window_id);
            inner.callbacks.key_event(&event, &window);
        }
    }

    extern "C" fn has_marked_text(_this: &mut Object, _sel: Sel) -> BOOL {
        NO
    }

    extern "C" fn marked_range(_this: &mut Object, _sel: Sel) -> NSRange {
        NSRange::new(NSNotFound as _, 0)
    }

    extern "C" fn selected_range(_this: &mut Object, _sel: Sel) -> NSRange {
        NSRange::new(NSNotFound as _, 0)
    }

    // Called by the IME when inserting composed text and/or emoji
    extern "C" fn insert_text_replacement_range(
        this: &mut Object,
        _sel: Sel,
        astring: id,
        _replacement_range: NSRange,
    ) {
        let s = unsafe { nsstring_to_str(astring) };
        if let Some(myself) = Self::get_this(this) {
            let mut inner = myself.inner.borrow_mut();
            let key_is_down = inner.key_is_down.take().unwrap_or(true);

            let event = KeyEvent {
                key: KeyCode::Composed(s.to_string()),
                raw_key: None,
                modifiers: Modifiers::NONE,
                raw_modifiers: Modifiers::NONE,
                raw_code: None,
                repeat_count: 1,
                key_is_down,
            }
            .normalize_shift();

            let window = Window(inner.window_id);
            inner.callbacks.key_event(&event, &window);
        }
    }

    extern "C" fn set_marked_text_selected_range_replacement_range(
        _this: &mut Object,
        _sel: Sel,
        _astring: id,
        selected_range: NSRange,
        replacement_range: NSRange,
    ) {
        let s = unsafe { nsstring_to_str(_astring) };
        eprintln!(
            "set_marked_text_selected_range_replacement_range {} {:?} {:?}",
            s, selected_range, replacement_range
        );
    }

    extern "C" fn unmark_text(_this: &mut Object, _sel: Sel) {
        eprintln!("unmarkText");
    }

    extern "C" fn valid_attributes_for_marked_text(_this: &mut Object, _sel: Sel) -> id {
        // FIXME: returns NSArray<NSAttributedStringKey> *
        // eprintln!("valid_attributes_for_marked_text");
        // nil
        unsafe { NSArray::arrayWithObjects(nil, &[]) }
    }

    extern "C" fn attributed_substring_for_proposed_range(
        _this: &mut Object,
        _sel: Sel,
        _proposed_range: NSRange,
        _actual_range: NSRangePointer,
    ) -> id {
        eprintln!(
            "attributedSubstringForProposedRange {:?} {:?}",
            _proposed_range, _actual_range
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
        eprintln!(
            "firstRectForCharacterRange: range:{:?} actual:{:?}",
            range, actual
        );
        let frame = unsafe {
            let window: id = msg_send![this, window];
            NSWindow::frame(window)
        };
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
                    frame.origin.x + cursor_pos.origin.x,
                    frame.origin.y + frame.size.height - cursor_pos.origin.y,
                ),
                NSSize::new(cursor_pos.size.width, cursor_pos.size.height),
            )
        } else {
            frame
        }
    }

    extern "C" fn accepts_first_responder(_this: &mut Object, _sel: Sel) -> BOOL {
        YES
    }

    extern "C" fn window_should_close(this: &mut Object, _sel: Sel, _id: id) -> BOOL {
        unsafe {
            let () = msg_send![this, setNeedsDisplay: YES];
        }

        if let Some(this) = Self::get_this(this) {
            if this.inner.borrow_mut().callbacks.can_close() {
                YES
            } else {
                NO
            }
        } else {
            YES
        }
    }

    extern "C" fn did_become_key(this: &mut Object, _sel: Sel, _id: id) {
        if let Some(this) = Self::get_this(this) {
            this.inner.borrow_mut().callbacks.focus_change(true);
        }
    }

    extern "C" fn did_resign_key(this: &mut Object, _sel: Sel, _id: id) {
        if let Some(this) = Self::get_this(this) {
            this.inner.borrow_mut().callbacks.focus_change(false);
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
            this.inner.borrow_mut().callbacks.destroy();
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
            coords = NSPoint::new(backing_rect.size.width, backing_rect.size.height);
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
            let window = Window(inner.window_id);
            inner.callbacks.mouse_event(&event, &window);
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

    extern "C" fn mouse_moved_or_dragged(this: &mut Object, _sel: Sel, nsevent: id) {
        Self::mouse_common(this, nsevent, MouseEventKind::Move);
    }

    fn key_common(this: &mut Object, nsevent: id, key_is_down: bool) {
        // let is_a_repeat = unsafe { nsevent.isARepeat() == YES };
        let chars = unsafe { nsstring_to_str(nsevent.characters()) };
        let unmod = unsafe { nsstring_to_str(nsevent.charactersIgnoringModifiers()) };
        let modifier_flags = unsafe { nsevent.modifierFlags() };
        let modifiers = key_modifiers(modifier_flags);
        let virtual_key = unsafe { nsevent.keyCode() };
        let translated;

        log::debug!(
            "key_common: chars=`{}` unmod=`{}` modifiers=`{:?}` virtual_key={:?} key_is_down:{}",
            chars.escape_debug(),
            unmod.escape_debug(),
            modifiers,
            virtual_key,
            key_is_down
        );

        let chars = if let Some(myself) = Self::get_this(this) {
            let mut inner = myself.inner.borrow_mut();

            if chars.is_empty() || inner.dead_pending.is_some() {
                // Dead key!
                if !key_is_down {
                    return;
                }

                match inner.translate_key_event(virtual_key, modifier_flags) {
                    None => {
                        // Next key press in dead key sequence is pending.
                        return;
                    }
                    Some(Ok(s)) => {
                        translated = s;
                        &translated
                    }
                    Some(Err(e)) => {
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

        let use_ime = config::configuration().use_ime;

        // `Delete` on macos is really Backspace and emits BS.
        // `Fn-Delete` emits DEL.
        // Alt-Delete is mapped by the IME to be equivalent to Fn-Delete.
        // We want to emit Alt-BS in that situation.
        let unmod =
            if virtual_key == super::keycodes::kVK_Delete && modifiers.contains(Modifiers::ALT) {
                "\x08"
            } else if virtual_key == super::keycodes::kVK_Tab {
                "\t"
            } else if !use_ime && virtual_key == super::keycodes::kVK_Delete {
                "\x08"
            } else {
                unmod
            };

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

        let only_alt = (modifiers & !(Modifiers::LEFT_ALT | Modifiers::RIGHT_ALT | Modifiers::ALT))
            == Modifiers::NONE;

        if key_is_down && use_ime && (modifiers.is_empty() || only_alt) {
            if let Some(myself) = Self::get_this(this) {
                let mut inner = myself.inner.borrow_mut();
                inner.key_is_down.replace(key_is_down);
            }

            unsafe {
                let input_context: id = msg_send![this, inputContext];
                let res: BOOL = msg_send![input_context, handleEvent: nsevent];
                if res == YES {
                    return;
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

        if let Some(key) = key_string_to_key_code(chars).or_else(|| key_string_to_key_code(unmod)) {
            let (key, raw_key) = if chars.is_empty() || chars == unmod {
                (key, None)
            } else {
                let raw = key_string_to_key_code(unmod);
                match (&key, &raw) {
                    // Avoid eg: \x01 when we can use CTRL-A
                    (KeyCode::Char(c), Some(raw)) if c.is_ascii_control() => (raw.clone(), None),
                    _ => (key, raw),
                }
            };

            let (modifiers, raw_modifiers) = if raw_key.is_some() {
                (Modifiers::NONE, modifiers)
            } else {
                (modifiers, Modifiers::NONE)
            };

            let event = KeyEvent {
                key,
                raw_key,
                modifiers,
                raw_modifiers,
                raw_code: Some(virtual_key as u32),
                repeat_count: 1,
                key_is_down,
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
                let window = Window(inner.window_id);
                inner.callbacks.key_event(&event, &window);
            }
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

    fn size_changed(&mut self) {
        let mut inner = self.inner.borrow_mut();
        let view = inner.view_id.as_ref().unwrap().load();
        let frame = unsafe { NSView::frame(*view) };
        let backing_frame = unsafe { NSView::convertRectToBacking(*view, frame) };
        let width = backing_frame.size.width;
        let height = backing_frame.size.height;

        // This is a little gross; ideally we'd call
        // WindowInner:is_fullscreen to determine this, but
        // we can't get a mutable reference to it from here
        // as we can be called in a context where something
        // higher up the callstack already has a mutable
        // reference and we'd panic.
        let is_fullscreen = inner.fullscreen.is_some()
            || inner.window.as_ref().map_or(false, |window| {
                let window = window.load();
                let style_mask = unsafe { NSWindow::styleMask(*window) };
                style_mask.contains(NSWindowStyleMask::NSFullScreenWindowMask)
            });

        if let Some(gpu_context) = inner.gpu_context.as_ref() {
            let mut gpu_context = gpu_context.borrow_mut();
            gpu_context.sc_desc.width = width as u32;
            gpu_context.sc_desc.height = height as u32;
        }

        inner.callbacks.resize(
            Dimensions {
                pixel_width: width as usize,
                pixel_height: height as usize,
                dpi: (crate::DEFAULT_DPI * (backing_frame.size.width / frame.size.width)) as usize,
            },
            is_fullscreen,
        );

        if let Some(gpu_context) = inner.gpu_context.as_ref() {
            let mut gpu_context = gpu_context.borrow_mut();
            gpu_context.swap_chain = gpu_context
                .device
                .create_swap_chain(&gpu_context.surface, &gpu_context.sc_desc);

            // We need to re-apply the setOpaque property, otherwise it
            // reverts back to opaque.
            unsafe {
                let layer: id = msg_send![*view, layer];
                let () = msg_send![layer, setOpaque: NO];
            }
        }
    }

    extern "C" fn did_resize(this: &mut Object, _sel: Sel, _notification: id) {
        if let Some(this) = Self::get_this(this) {
            this.size_changed();
        }
    }
    extern "C" fn wants_update_layer(_view: &mut Object, _sel: Sel) -> BOOL {
        YES
    }

    extern "C" fn update_layer(_view: &mut Object, _sel: Sel) {
        log::info!("update_layer called");
    }

    extern "C" fn display_layer(view: &mut Object, _sel: Sel, _layer: id) {
        if let Some(this) = Self::get_this(view) {
            this.call_out_to_render();
        }
    }

    extern "C" fn draw_layer_in_context(_view: &mut Object, _sel: Sel, _layer: id, _context: id) {
        log::info!("draw_layer_in_context called");
    }

    extern "C" fn layer_should_inherit_contents_scale_from_window(
        _: &Object,
        _: Sel,
        _: *mut Object,
        _: CGFloat,
        _: *mut Object,
    ) -> BOOL {
        YES
    }

    extern "C" fn make_backing_layer(view: &mut Object, _: Sel) -> id {
        let class = class!(CAMetalLayer);
        unsafe {
            let layer: id = msg_send![class, new];
            let () = msg_send![layer, setOpaque: NO];
            let () = msg_send![layer, setDelegate: view];
            layer
        }
    }

    extern "C" fn draw_rect(view: &mut Object, _sel: Sel, _dirty_rect: NSRect) {
        if let Some(this) = Self::get_this(view) {
            this.call_out_to_render();
        }
    }

    fn call_out_to_render(&mut self) {
        let mut inner = self.inner.borrow_mut();

        if inner.screen_changed {
            // If the screen resolution changed (which can also
            // happen if the window was dragged to another monitor
            // with different dpi), then we treat this as a resize
            // event that will in turn trigger an invalidation
            // and a repaint.
            inner.screen_changed = false;
            log::info!("screen changed, so trigger resize");
            drop(inner);
            self.size_changed();
            return;
        }

        if let Some(gpu_context) = inner.gpu_context.as_ref().cloned() {
            let mut gpu_context = gpu_context.borrow_mut();
            let frame = match gpu_context.swap_chain.get_current_frame() {
                Ok(frame) => frame,
                Err(err) => {
                    log::info!("get_current_frame: {:#}", err);
                    gpu_context.swap_chain = gpu_context
                        .device
                        .create_swap_chain(&gpu_context.surface, &gpu_context.sc_desc);
                    gpu_context
                        .swap_chain
                        .get_current_frame()
                        .expect("Failed to acquire next swap chain texture!")
                }
            };

            inner.callbacks.render(&frame.output, &mut *gpu_context);
        } else {
            log::info!("there is no gpu context");
        }
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
        cls.add_protocol(
            Protocol::get("NSViewLayerContentScaleDelegate")
                .expect("NSViewLayerContentScaleDelegate not defined"),
        );
        cls.add_protocol(Protocol::get("CALayerDelegate").expect("CALayerDelegate not defined"));

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
                sel!(makeBackingLayer),
                Self::make_backing_layer as extern "C" fn(&mut Object, Sel) -> id,
            );

            cls.add_method(
                sel!(drawRect:),
                Self::draw_rect as extern "C" fn(&mut Object, Sel, NSRect),
            );

            cls.add_method(
                sel!(layer:shouldInheritContentsScale:fromWindow:),
                Self::layer_should_inherit_contents_scale_from_window
                    as extern "C" fn(&Object, Sel, *mut Object, CGFloat, *mut Object) -> BOOL,
            );

            cls.add_method(
                sel!(updateLayer),
                Self::update_layer as extern "C" fn(&mut Object, Sel),
            );

            cls.add_method(
                sel!(displayLayer:),
                Self::display_layer as extern "C" fn(&mut Object, Sel, id),
            );

            cls.add_method(
                sel!(drawLayer:inContext:),
                Self::draw_layer_in_context as extern "C" fn(&mut Object, Sel, id, id),
            );
            cls.add_method(
                sel!(wantsUpdateLayer),
                Self::wants_update_layer as extern "C" fn(&mut Object, Sel) -> BOOL,
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
                sel!(scrollWheel:),
                Self::scroll_wheel as extern "C" fn(&mut Object, Sel, id),
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
                sel!(acceptsFirstResponder),
                Self::accepts_first_responder as extern "C" fn(&mut Object, Sel) -> BOOL,
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
        }

        cls.register()
    }
}
