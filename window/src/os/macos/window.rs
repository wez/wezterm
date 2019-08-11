use super::nsstring;
use crate::bitmaps::Image;
use crate::os::macos::bitmap::BitmapRef;
use crate::{BitmapImage, Color, Dimensions, Operator, PaintContext, WindowCallbacks};
use cocoa::appkit::{
    NSApplicationActivateIgnoringOtherApps, NSBackingStoreBuffered, NSRunningApplication, NSView,
    NSViewHeightSizable, NSViewWidthSizable, NSWindow, NSWindowStyleMask,
};
use cocoa::base::*;
use cocoa::foundation::{NSPoint, NSRect, NSSize};
use core_graphics::image::CGImageRef;
use failure::Fallible;
use objc::declare::ClassDecl;
use objc::rc::{StrongPtr, WeakPtr};
use objc::runtime::{Class, Object, Sel};
use objc::*;
use std::ffi::c_void;
use std::sync::{Arc, Mutex};

pub struct Window {
    window: StrongPtr,
    view: StrongPtr,
}

impl Drop for Window {
    fn drop(&mut self) {
        eprintln!("drop Window");
    }
}

impl Window {
    pub fn new_window(
        _class_name: &str,
        name: &str,
        width: usize,
        height: usize,
        callbacks: Box<WindowCallbacks>,
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

            let inner = Arc::new(Mutex::new(Inner {
                callbacks,
                view_id: None,
            }));

            let window = StrongPtr::new(
                NSWindow::alloc(nil).initWithContentRect_styleMask_backing_defer_(
                    rect,
                    style_mask,
                    NSBackingStoreBuffered,
                    NO,
                ),
            );

            window.cascadeTopLeftFromPoint_(NSPoint::new(20.0, 20.0));
            window.setTitle_(*nsstring(&name));
            window.setAcceptsMouseMovedEvents_(YES);

            let view = WindowView::alloc(&inner)?;
            view.initWithFrame_(rect);
            view.setAutoresizingMask_(NSViewHeightSizable | NSViewWidthSizable);

            window.setContentView_(*view);
            window.setDelegate_(*view);

            Ok(Self { window, view })
        }
    }

    pub fn show(&self) {
        unsafe {
            let current_app = NSRunningApplication::currentApplication(nil);
            current_app.activateWithOptions_(NSApplicationActivateIgnoringOtherApps);
            self.window.makeKeyAndOrderFront_(nil)
        }
    }
}

struct Inner {
    callbacks: Box<WindowCallbacks>,
    view_id: Option<WeakPtr>,
}

impl Drop for Inner {
    fn drop(&mut self) {
        eprintln!("dropping Inner");
    }
}

const CLS_NAME: &str = "WezTermWindowView";

struct WindowView {
    inner: Arc<Mutex<Inner>>,
}

impl Drop for WindowView {
    fn drop(&mut self) {
        eprintln!("dropping WindowView");
    }
}

pub fn superclass(this: &Object) -> &'static Class {
    unsafe {
        let superclass: id = msg_send![this, superclass];
        &*(superclass as *const _)
    }
}

struct MacGraphicsContext<'a> {
    buffer: &'a mut BitmapImage,
}

impl<'a> PaintContext for MacGraphicsContext<'a> {
    fn clear_rect(
        &mut self,
        dest_x: isize,
        dest_y: isize,
        width: usize,
        height: usize,
        color: Color,
    ) {
        self.buffer.clear_rect(dest_x, dest_y, width, height, color)
    }

    fn clear(&mut self, color: Color) {
        self.buffer.clear(color);
    }

    fn get_dimensions(&self) -> Dimensions {
        let (pixel_width, pixel_height) = self.buffer.image_dimensions();
        Dimensions {
            pixel_width,
            pixel_height,
            dpi: 96,
        }
    }

    fn draw_image_subset(
        &mut self,
        dest_x: isize,
        dest_y: isize,
        src_x: usize,
        src_y: usize,
        width: usize,
        height: usize,
        im: &dyn BitmapImage,
        operator: Operator,
    ) {
        self.buffer
            .draw_image_subset(dest_x, dest_y, src_x, src_y, width, height, im, operator)
    }
}

impl WindowView {
    fn view_id(&self) -> id {
        let inner = self.inner.lock().unwrap();
        inner.view_id.as_ref().map(|w| *w.load()).unwrap_or(nil)
    }

    fn window_id(&self) -> id {
        unsafe { msg_send![self.view_id(), window] }
    }

    extern "C" fn dealloc(this: &mut Object, _sel: Sel) {
        eprintln!("WindowView::dealloc");
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

    extern "C" fn window_should_close(this: &mut Object, _sel: Sel, _id: id) -> BOOL {
        eprintln!("window_should_close");

        unsafe {
            let () = msg_send![this, setNeedsDisplay: YES];
        }

        if let Some(this) = Self::get_this(this) {
            if this.inner.lock().unwrap().callbacks.can_close() {
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
        eprintln!("window_will_close");
        if let Some(this) = Self::get_this(this) {
            // Advise the window of its impending death
            this.inner.lock().unwrap().callbacks.destroy();
        }

        // Release and zero out the inner member
        Self::drop_inner(this);
    }

    extern "C" fn draw_rect(this: &mut Object, _sel: Sel, _dirty_rect: NSRect) {
        let frame = unsafe { NSView::frame(this as *mut _) };
        let width = frame.size.width;
        let height = frame.size.height;

        let mut im = Image::new(width as usize, height as usize);

        if let Some(this) = Self::get_this(this) {
            let mut ctx = MacGraphicsContext { buffer: &mut im };
            this.inner.lock().unwrap().callbacks.paint(&mut ctx);
        }

        let cg_image = BitmapRef::with_image(&im);

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

    fn alloc(inner: &Arc<Mutex<Inner>>) -> Fallible<StrongPtr> {
        let cls = Self::get_class();

        let view_id: StrongPtr = unsafe { StrongPtr::new(msg_send![cls, new]) };

        inner.lock().unwrap().view_id.replace(view_id.weak());

        let view = Box::into_raw(Box::new(Self {
            inner: Arc::clone(&inner),
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
        }

        cls.register()
    }
}
