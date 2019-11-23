// let () = msg_send! is a common pattern for objc
#![allow(clippy::let_unit_value)]

use super::{nsstring, nsstring_to_str};
use crate::bitmaps::Image;
use crate::connection::ConnectionOps;
use crate::os::macos::bitmap::BitmapRef;
use crate::{
    BitmapImage, Color, Connection, Dimensions, KeyCode, KeyEvent, Modifiers, MouseButtons,
    MouseCursor, MouseEvent, MouseEventKind, MousePress, Operator, PaintContext, Point, Rect, Size,
    WindowCallbacks, WindowOps, WindowOpsMut,
};
use cocoa::appkit::{
    NSApplicationActivateIgnoringOtherApps, NSBackingStoreBuffered, NSEvent, NSEventModifierFlags,
    NSRunningApplication, NSView, NSViewHeightSizable, NSViewWidthSizable, NSWindow,
    NSWindowStyleMask,
};
use cocoa::base::*;
use cocoa::foundation::{NSArray, NSNotFound, NSPoint, NSRect, NSSize, NSUInteger};
use core_graphics::image::CGImageRef;
use failure::Fallible;
use objc::declare::ClassDecl;
use objc::rc::{StrongPtr, WeakPtr};
use objc::runtime::{Class, Object, Protocol, Sel};
use objc::*;
use promise::Future;
use std::any::Any;
use std::cell::RefCell;
use std::ffi::c_void;
use std::rc::Rc;

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

#[cfg(feature = "opengl")]
mod opengl {
    use super::*;
    use cocoa::appkit::{self, NSOpenGLContext, NSOpenGLPixelFormat};
    use cocoa::foundation::NSAutoreleasePool;
    use core_foundation::base::TCFType;
    use core_foundation::bundle::{
        CFBundleGetBundleWithIdentifier, CFBundleGetFunctionPointerForName,
    };
    use core_foundation::string::CFString;
    use std::str::FromStr;

    #[derive(Clone)]
    pub struct GlContextPair {
        pub context: Rc<glium::backend::Context>,
        pub backend: Rc<GlState>,
    }

    impl GlContextPair {
        pub fn create(view: id) -> Fallible<Self> {
            let backend = Rc::new(GlState::create(view)?);

            let context = unsafe {
                glium::backend::Context::new(
                    Rc::clone(&backend),
                    true,
                    if cfg!(debug_assertions) {
                        glium::debug::DebugCallbackBehavior::DebugMessageOnError
                    } else {
                        glium::debug::DebugCallbackBehavior::Ignore
                    },
                )
            }?;

            Ok(Self { context, backend })
        }
    }

    pub struct GlState {
        _pixel_format: StrongPtr,
        gl_context: StrongPtr,
    }

    impl GlState {
        pub fn create(view: id) -> Fallible<Self> {
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
            failure::ensure!(
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
            failure::ensure!(!gl_context.is_null(), "failed to create NSOpenGLContext");
            unsafe {
                gl_context.setView_(view);
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
    window_id: usize,
    view: StrongPtr,
    window: StrongPtr,
}

fn function_key_to_keycode(function_key: char) -> KeyCode {
    // FIXME: CTRL-C is 0x3, should it be normalized to C here
    // using the unmod string?  Or should be normalize the 0x3
    // as the canonical representation of that input?
    use cocoa::appkit;
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
    pub fn new_window(
        _class_name: &str,
        name: &str,
        width: usize,
        height: usize,
        callbacks: Box<dyn WindowCallbacks>,
    ) -> Fallible<Window> {
        unsafe {
            let style_mask = NSWindowStyleMask::NSTitledWindowMask
                | NSWindowStyleMask::NSClosableWindowMask
                | NSWindowStyleMask::NSMiniaturizableWindowMask
                | NSWindowStyleMask::NSResizableWindowMask;
            let rect = NSRect::new(
                NSPoint::new(0., 0.),
                NSSize::new(width as f64, height as f64),
            );

            let conn = Connection::get().expect("Connection::init has not been called");

            let window_id = conn.next_window_id();

            let inner = Rc::new(RefCell::new(Inner {
                callbacks,
                view_id: None,
                window_id,
                #[cfg(feature = "opengl")]
                gl_context_pair: None,
                text_cursor_position: Rect::new(Point::new(0, 0), Size::new(0, 0)),
            }));

            let window = StrongPtr::new(
                NSWindow::alloc(nil).initWithContentRect_styleMask_backing_defer_(
                    rect,
                    style_mask,
                    NSBackingStoreBuffered,
                    NO,
                ),
            );

            window.setReleasedWhenClosed_(NO);
            // window.cascadeTopLeftFromPoint_(NSPoint::new(20.0, 20.0));
            window.center();
            window.setTitle_(*nsstring(&name));
            window.setAcceptsMouseMovedEvents_(YES);

            let buffer = Image::new(width, height);
            let view = WindowView::alloc(&inner, buffer)?;
            view.initWithFrame_(rect);
            view.setAutoresizingMask_(NSViewHeightSizable | NSViewWidthSizable);

            window.setContentView_(*view);
            window.setDelegate_(*view);

            let frame = NSView::frame(*view);
            let backing_frame = NSView::convertRectToBacking(*view, frame);
            let width = backing_frame.size.width;
            let height = backing_frame.size.height;

            let window_inner = Rc::new(RefCell::new(WindowInner {
                window_id,
                window,
                view,
            }));
            conn.windows
                .borrow_mut()
                .insert(window_id, Rc::clone(&window_inner));

            let window = Window(window_id);

            inner.borrow_mut().callbacks.created(&window);
            // Synthesize a resize event immediately; this allows
            // the embedding application an opportunity to discover
            // the dpi and adjust for display scaling
            inner.borrow_mut().callbacks.resize(Dimensions {
                pixel_width: width as usize,
                pixel_height: height as usize,
                dpi: (96.0 * (backing_frame.size.width / frame.size.width)) as usize,
            });

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

    fn set_text_cursor_position(&self, cursor: Rect) -> Future<()> {
        Connection::with_window_inner(self.0, move |inner| {
            inner.set_text_cursor_position(cursor);
            Ok(())
        })
    }

    fn apply<R, F: Send + 'static + Fn(&mut dyn Any, &dyn WindowOps) -> Fallible<R>>(
        &self,
        func: F,
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
                failure::bail!("apply: window is invalid");
            }
        })
    }

    #[cfg(feature = "opengl")]
    fn enable_opengl<
        R,
        F: Send
            + 'static
            + Fn(
                &mut dyn Any,
                &dyn WindowOps,
                failure::Fallible<std::rc::Rc<glium::backend::Context>>,
            ) -> failure::Fallible<R>,
    >(
        &self,
        func: F,
    ) -> promise::Future<R>
    where
        Self: Sized,
        R: Send + 'static,
    {
        Connection::with_window_inner(self.0, move |inner| {
            let window = Window(inner.window_id);

            let glium_context = opengl::GlContextPair::create(*inner.view);

            if let Some(window_view) = WindowView::get_this(unsafe { &**inner.view }) {
                window_view.inner.borrow_mut().gl_context_pair =
                    glium_context.as_ref().map(Clone::clone).ok();

                func(
                    window_view.inner.borrow_mut().callbacks.as_any(),
                    &window,
                    glium_context.map(|pair| pair.context),
                )
            } else {
                failure::bail!("enable_opengl: window is invalid");
            }
        })
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

    fn hide(&mut self) {}

    fn set_cursor(&mut self, cursor: Option<MouseCursor>) {
        unsafe {
            let ns_cursor_cls = class!(NSCursor);
            if let Some(cursor) = cursor {
                let instance: id = match cursor {
                    MouseCursor::Arrow => msg_send![ns_cursor_cls, arrowCursor],
                    MouseCursor::Text => msg_send![ns_cursor_cls, IBeamCursor],
                    MouseCursor::Hand => msg_send![ns_cursor_cls, pointingHandCursor],
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

    fn set_inner_size(&self, width: usize, height: usize) {
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

    fn set_text_cursor_position(&mut self, cursor: Rect) {
        if let Some(window_view) = WindowView::get_this(unsafe { &**self.view }) {
            window_view.inner.borrow_mut().text_cursor_position = cursor;
        }
        unsafe {
            let input_context: id = msg_send![&**self.view, inputContext];
            let () = msg_send![input_context, invalidateCharacterCoordinates];
        }
    }
}

struct Inner {
    callbacks: Box<dyn WindowCallbacks>,
    view_id: Option<WeakPtr>,
    window_id: usize,
    #[cfg(feature = "opengl")]
    gl_context_pair: Option<opengl::GlContextPair>,
    text_cursor_position: Rect,
}

const CLS_NAME: &str = "WezTermWindowView";

struct WindowView {
    inner: Rc<RefCell<Inner>>,
    buffer: RefCell<Image>,
}

pub fn superclass(this: &Object) -> &'static Class {
    unsafe {
        let superclass: id = msg_send![this, superclass];
        &*(superclass as *const _)
    }
}

struct MacGraphicsContext<'a> {
    buffer: &'a mut dyn BitmapImage,
    dpi: usize,
}

impl<'a> PaintContext for MacGraphicsContext<'a> {
    fn clear_rect(&mut self, rect: Rect, color: Color) {
        self.buffer.clear_rect(rect, color)
    }

    fn clear(&mut self, color: Color) {
        self.buffer.clear(color);
    }

    fn get_dimensions(&self) -> Dimensions {
        let (pixel_width, pixel_height) = self.buffer.image_dimensions();
        Dimensions {
            pixel_width,
            pixel_height,
            dpi: self.dpi,
        }
    }

    fn draw_image(
        &mut self,
        dest_top_left: Point,
        src_rect: Option<Rect>,
        im: &dyn BitmapImage,
        operator: Operator,
    ) {
        self.buffer
            .draw_image(dest_top_left, src_rect, im, operator)
    }

    fn draw_line(&mut self, start: Point, end: Point, color: Color, operator: Operator) {
        self.buffer.draw_line(start, end, color, operator);
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
    if flags.contains(NSEventModifierFlags::NSAlternateKeyMask) {
        mods |= Modifiers::ALT;
    }
    if flags.contains(NSEventModifierFlags::NSControlKeyMask) {
        mods |= Modifiers::CTRL;
    }
    if flags.contains(NSEventModifierFlags::NSCommandKeyMask) {
        mods |= Modifiers::SUPER;
    }

    mods
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
            let myself: *mut c_void = *this.get_ivar(CLS_NAME);
            this.set_ivar(CLS_NAME, std::ptr::null_mut() as *mut c_void);

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
            repeat_count: 1,
            key_is_down: true,
        };

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

        let event = KeyEvent {
            key: KeyCode::Composed(s.to_string()),
            raw_key: None,
            modifiers: Modifiers::default(),
            repeat_count: 1,
            key_is_down: true,
        };

        if let Some(myself) = Self::get_this(this) {
            let mut inner = myself.inner.borrow_mut();
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

    // Switch the coordinate system to have 0,0 in the top left
    extern "C" fn is_flipped(_this: &Object, _sel: Sel) -> BOOL {
        YES
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
        unsafe {
            let point = NSView::convertPoint_fromView_(view, nsevent.locationInWindow(), nil);
            let rect = NSRect::new(NSPoint::new(0., 0.), NSSize::new(point.x, point.y));
            let backing_rect = NSView::convertRectToBacking(view, rect);
            coords = NSPoint::new(backing_rect.size.width, backing_rect.size.height);
            mouse_buttons = decode_mouse_buttons(NSEvent::pressedMouseButtons(nsevent));
            modifiers = key_modifiers(nsevent.modifierFlags());
        }
        let event = MouseEvent {
            kind,
            x: coords.x.max(0.0) as u16,
            y: coords.y.max(0.0) as u16,
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
        let vert_delta = unsafe { nsevent.scrollingDeltaY() };
        let horz_delta = unsafe { nsevent.scrollingDeltaX() };
        let kind = if vert_delta.abs() > horz_delta.abs() {
            MouseEventKind::VertWheel(vert_delta as i16)
        } else {
            MouseEventKind::HorzWheel(horz_delta as i16)
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
        let is_a_repeat = unsafe { nsevent.isARepeat() == YES };
        let chars = unsafe { nsstring_to_str(nsevent.characters()) };
        let unmod = unsafe { nsstring_to_str(nsevent.charactersIgnoringModifiers()) };
        let modifiers = unsafe { key_modifiers(nsevent.modifierFlags()) };
        let virtual_key = unsafe { nsevent.keyCode() };

        // `Delete` on macos is really Backspace and emits BS.
        // `Fn-Delete` emits DEL.
        // Alt-Delete is mapped by the IME to be equivalent to Fn-Delete.
        // We want to emit Alt-BS in that situation.
        let unmod =
            if virtual_key == super::keycodes::kVK_Delete && modifiers.contains(Modifiers::ALT) {
                "\x08"
            } else if virtual_key == super::keycodes::kVK_Tab {
                "\t"
            } else {
                unmod
            };

        if modifiers.is_empty() && !is_a_repeat {
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

        fn normalize_shifted_unmodified_key(kc: KeyCode, virtual_key: u16) -> KeyCode {
            use super::keycodes;
            match virtual_key {
                keycodes::kVK_ANSI_A => KeyCode::Char('a'),
                keycodes::kVK_ANSI_B => KeyCode::Char('b'),
                keycodes::kVK_ANSI_C => KeyCode::Char('c'),
                keycodes::kVK_ANSI_D => KeyCode::Char('d'),
                keycodes::kVK_ANSI_E => KeyCode::Char('e'),
                keycodes::kVK_ANSI_F => KeyCode::Char('f'),
                keycodes::kVK_ANSI_G => KeyCode::Char('g'),
                keycodes::kVK_ANSI_H => KeyCode::Char('h'),
                keycodes::kVK_ANSI_I => KeyCode::Char('i'),
                keycodes::kVK_ANSI_J => KeyCode::Char('j'),
                keycodes::kVK_ANSI_K => KeyCode::Char('k'),
                keycodes::kVK_ANSI_L => KeyCode::Char('l'),
                keycodes::kVK_ANSI_M => KeyCode::Char('m'),
                keycodes::kVK_ANSI_N => KeyCode::Char('n'),
                keycodes::kVK_ANSI_O => KeyCode::Char('o'),
                keycodes::kVK_ANSI_P => KeyCode::Char('p'),
                keycodes::kVK_ANSI_Q => KeyCode::Char('q'),
                keycodes::kVK_ANSI_R => KeyCode::Char('r'),
                keycodes::kVK_ANSI_S => KeyCode::Char('s'),
                keycodes::kVK_ANSI_T => KeyCode::Char('t'),
                keycodes::kVK_ANSI_U => KeyCode::Char('u'),
                keycodes::kVK_ANSI_V => KeyCode::Char('v'),
                keycodes::kVK_ANSI_W => KeyCode::Char('w'),
                keycodes::kVK_ANSI_X => KeyCode::Char('x'),
                keycodes::kVK_ANSI_Y => KeyCode::Char('y'),
                keycodes::kVK_ANSI_Z => KeyCode::Char('z'),
                keycodes::kVK_ANSI_0 => KeyCode::Char('0'),
                keycodes::kVK_ANSI_1 => KeyCode::Char('1'),
                keycodes::kVK_ANSI_2 => KeyCode::Char('2'),
                keycodes::kVK_ANSI_3 => KeyCode::Char('3'),
                keycodes::kVK_ANSI_4 => KeyCode::Char('4'),
                keycodes::kVK_ANSI_5 => KeyCode::Char('5'),
                keycodes::kVK_ANSI_6 => KeyCode::Char('6'),
                keycodes::kVK_ANSI_7 => KeyCode::Char('7'),
                keycodes::kVK_ANSI_8 => KeyCode::Char('8'),
                keycodes::kVK_ANSI_9 => KeyCode::Char('9'),
                keycodes::kVK_ANSI_Equal => KeyCode::Char('='),
                keycodes::kVK_ANSI_Minus => KeyCode::Char('-'),
                keycodes::kVK_ANSI_LeftBracket => KeyCode::Char('['),
                keycodes::kVK_ANSI_RightBracket => KeyCode::Char(']'),
                keycodes::kVK_ANSI_Quote => KeyCode::Char('\''),
                keycodes::kVK_ANSI_Semicolon => KeyCode::Char(';'),
                keycodes::kVK_ANSI_Backslash => KeyCode::Char('\\'),
                keycodes::kVK_ANSI_Comma => KeyCode::Char(','),
                keycodes::kVK_ANSI_Slash => KeyCode::Char('/'),
                keycodes::kVK_ANSI_Period => KeyCode::Char('.'),
                keycodes::kVK_ANSI_Grave => KeyCode::Char('`'),

                keycodes::kVK_Delete => KeyCode::Char('\x08'),
                keycodes::kVK_ForwardDelete => KeyCode::Char('\x7f'),
                _ => kc,
            }
        }

        if let Some(key) = key_string_to_key_code(chars) {
            let (key, raw_key) = if chars == unmod {
                (key, None)
            } else {
                let raw = key_string_to_key_code(unmod)
                    .map(|kc| normalize_shifted_unmodified_key(kc, virtual_key));

                match (&key, &raw) {
                    (KeyCode::Char(c), Some(raw)) if c.is_ascii_control() => (raw.clone(), None),
                    _ => (key, raw),
                }
            };

            let event = KeyEvent {
                key,
                raw_key,
                modifiers,
                repeat_count: 1,
                key_is_down,
            };

            /*
            eprintln!(
                "key_common {:?} (chars={:?} unmod={:?} modifiers={:?}",
                event, chars, unmod, modifiers
            );
            */

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

    /*
    extern "C" fn key_up(this: &mut Object, _sel: Sel, nsevent: id) {
        Self::key_common(this, nsevent, false);
    }
    */

    extern "C" fn did_resize(this: &mut Object, _sel: Sel, _notification: id) {
        #[cfg(feature = "opengl")]
        {
            if let Some(this) = Self::get_this(this) {
                let inner = this.inner.borrow_mut();

                if let Some(gl_context_pair) = inner.gl_context_pair.as_ref() {
                    gl_context_pair.backend.update();
                }
            }
        }

        let frame = unsafe { NSView::frame(this as *mut _) };
        let backing_frame = unsafe { NSView::convertRectToBacking(this as *mut _, frame) };
        let width = backing_frame.size.width;
        let height = backing_frame.size.height;
        if let Some(this) = Self::get_this(this) {
            this.inner.borrow_mut().callbacks.resize(Dimensions {
                pixel_width: width as usize,
                pixel_height: height as usize,
                dpi: (96.0 * (backing_frame.size.width / frame.size.width)) as usize,
            });
        }
    }

    extern "C" fn draw_rect(this: &mut Object, _sel: Sel, _dirty_rect: NSRect) {
        let frame = unsafe { NSView::frame(this as *mut _) };
        let backing_frame = unsafe { NSView::convertRectToBacking(this as *mut _, frame) };

        let width = backing_frame.size.width;
        let height = backing_frame.size.height;

        if let Some(this) = Self::get_this(this) {
            let mut inner = this.inner.borrow_mut();
            let mut buffer = this.buffer.borrow_mut();

            #[cfg(feature = "opengl")]
            {
                if let Some(gl_context_pair) = inner.gl_context_pair.as_ref() {
                    let mut frame = glium::Frame::new(
                        Rc::clone(&gl_context_pair.context),
                        (width as u32, height as u32),
                    );

                    inner.callbacks.paint_opengl(&mut frame);
                    frame
                        .finish()
                        .expect("frame.finish failed and we don't know how to recover");
                    return;
                }
            }

            let (pixel_width, pixel_height) = buffer.image_dimensions();
            if width as usize != pixel_width || height as usize != pixel_height {
                *buffer = Image::new(width as usize, height as usize);
            }

            let mut ctx = MacGraphicsContext {
                buffer: &mut *buffer,
                dpi: (96.0 * backing_frame.size.width / frame.size.width) as usize,
            };

            inner.callbacks.paint(&mut ctx);

            let cg_image = BitmapRef::with_image(&*buffer);

            fn nsimage_from_cgimage(cg: &CGImageRef, size: NSSize) -> StrongPtr {
                unsafe {
                    let ns_image: id = msg_send![class!(NSImage), alloc];
                    StrongPtr::new(msg_send![ns_image, initWithCGImage: cg size:size])
                }
            }

            let ns_image = nsimage_from_cgimage(cg_image.as_ref(), NSSize::new(0., 0.));

            unsafe {
                let () = msg_send![*ns_image, drawInRect: frame];
            }
        }
    }

    fn get_this(this: &Object) -> Option<&mut Self> {
        unsafe {
            let myself: *mut c_void = *this.get_ivar(CLS_NAME);
            if myself.is_null() {
                None
            } else {
                Some(&mut *(myself as *mut Self))
            }
        }
    }

    fn alloc(inner: &Rc<RefCell<Inner>>, buffer: Image) -> Fallible<StrongPtr> {
        let cls = Self::get_class();

        let view_id: StrongPtr = unsafe { StrongPtr::new(msg_send![cls, new]) };

        inner.borrow_mut().view_id.replace(view_id.weak());

        let view = Box::into_raw(Box::new(Self {
            inner: Rc::clone(&inner),
            buffer: RefCell::new(buffer),
        }));

        unsafe {
            (**view_id).set_ivar(CLS_NAME, view as *mut c_void);
        }

        Ok(view_id)
    }

    fn get_class() -> &'static Class {
        Class::get(CLS_NAME).unwrap_or_else(Self::define_class)
    }

    fn define_class() -> &'static Class {
        let mut cls =
            ClassDecl::new(CLS_NAME, class!(NSView)).expect("Unable to register WindowView class");

        cls.add_ivar::<*mut c_void>(CLS_NAME);
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
                sel!(windowDidResize:),
                Self::did_resize as extern "C" fn(&mut Object, Sel, id),
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
            /* keyUp events mess up the IME and we generally only care
             * about the down events anyway.  Leaving this un-plumbed
             * means that we'll fall back to the default behavior for
             * keyUp which helps make key repeat work.
            cls.add_method(
                sel!(keyUp:),
                Self::key_up as extern "C" fn(&mut Object, Sel, id),
            );
            */

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
