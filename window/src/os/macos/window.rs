use super::{nsstring, nsstring_to_str};
use crate::bitmaps::Image;
use crate::connection::ConnectionOps;
use crate::os::macos::bitmap::BitmapRef;
use crate::{
    BitmapImage, Color, Connection, Dimensions, KeyCode, KeyEvent, Modifiers, MouseButtons,
    MouseCursor, MouseEvent, MouseEventKind, MousePress, Operator, PaintContext, Point, Rect,
    WindowCallbacks, WindowOps, WindowOpsMut,
};
use cocoa::appkit::{
    NSApplicationActivateIgnoringOtherApps, NSBackingStoreBuffered, NSEvent, NSEventModifierFlags,
    NSRunningApplication, NSView, NSViewHeightSizable, NSViewWidthSizable, NSWindow,
    NSWindowStyleMask,
};
use cocoa::base::*;
use cocoa::foundation::{NSPoint, NSRect, NSSize};
use core_graphics::image::CGImageRef;
use failure::Fallible;
use objc::declare::ClassDecl;
use objc::rc::{StrongPtr, WeakPtr};
use objc::runtime::{Class, Object, Sel};
use objc::*;
use std::any::Any;
use std::cell::RefCell;
use std::ffi::c_void;
use std::rc::Rc;

pub(crate) struct WindowInner {
    window_id: usize,
    view: StrongPtr,
    window: StrongPtr,
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
            window.cascadeTopLeftFromPoint_(NSPoint::new(20.0, 20.0));
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
    fn hide(&self) {
        Connection::with_window_inner(self.0, |inner| inner.hide());
    }
    fn show(&self) {
        Connection::with_window_inner(self.0, |inner| inner.show());
    }
    fn set_cursor(&self, cursor: Option<MouseCursor>) {
        Connection::with_window_inner(self.0, move |inner| {
            let _ = inner.set_cursor(cursor);
        });
    }
    fn invalidate(&self) {
        Connection::with_window_inner(self.0, |inner| inner.invalidate());
    }
    fn set_title(&self, title: &str) {
        let title = title.to_owned();
        Connection::with_window_inner(self.0, move |inner| inner.set_title(&title));
    }

    fn apply<F: Send + 'static + Fn(&mut dyn Any, &dyn WindowOps)>(&self, func: F)
    where
        Self: Sized,
    {
        Connection::with_window_inner(self.0, move |inner| {
            let window = Window(inner.window_id);

            if let Some(window_view) = WindowView::get_this(unsafe { &**inner.view }) {
                func(window_view.inner.borrow_mut().callbacks.as_any(), &window);
            }
        });
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
}

struct Inner {
    callbacks: Box<dyn WindowCallbacks>,
    view_id: Option<WeakPtr>,
    window_id: usize,
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
            coords = NSView::convertPoint_fromView_(view, nsevent.locationInWindow(), nil);
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
        let chars = unsafe { nsstring_to_str(nsevent.characters()) };
        // let unmod = unsafe {nsstring_to_str(nsevent.charactersIgnoringModifiers())};
        let modifiers = unsafe { key_modifiers(nsevent.modifierFlags()) };

        let mut char_iter = chars.chars();
        if let Some(first_char) = char_iter.next() {
            let key = if char_iter.next().is_none() {
                // A single unicode char
                match first_char {
                    '\u{f700}' => KeyCode::UpArrow,
                    '\u{f701}' => KeyCode::DownArrow,
                    '\u{f702}' => KeyCode::LeftArrow,
                    '\u{f703}' => KeyCode::RightArrow,
                    '\u{f729}' => KeyCode::Home,
                    '\u{f72b}' => KeyCode::End,
                    '\u{f72c}' => KeyCode::PageUp,
                    '\u{f72d}' => KeyCode::PageDown,
                    _ => KeyCode::Char(first_char),
                }
            } else {
                KeyCode::Composed(chars.to_owned())
            };

            // FIXME: CTRL-C is 0x3, should it be normalized to C here
            // using the unmod string?  Or should be normalize the 0x3
            // as the canonical representation of that input?

            let event = KeyEvent {
                key,
                modifiers,
                repeat_count: 1,
                key_is_down,
            };

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

    extern "C" fn did_resize(this: &mut Object, _sel: Sel, _notification: id) {
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

            let (pixel_width, pixel_height) = buffer.image_dimensions();
            if width as usize != pixel_width || height as usize != pixel_height {
                *buffer = Image::new(width as usize, height as usize);
            }

            let mut ctx = MacGraphicsContext {
                buffer: &mut *buffer,
                dpi: (96.0 * backing_frame.size.width / frame.size.width) as usize,
            };

            inner.callbacks.paint(&mut ctx);

            let cg_image = BitmapRef::with_image(&mut *buffer);

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
            cls.add_method(
                sel!(keyUp:),
                Self::key_up as extern "C" fn(&mut Object, Sel, id),
            );

            cls.add_method(
                sel!(acceptsFirstResponder),
                Self::accepts_first_responder as extern "C" fn(&mut Object, Sel) -> BOOL,
            );
        }

        cls.register()
    }
}
