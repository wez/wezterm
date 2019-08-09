use super::nsstring;
use crate::WindowCallbacks;
use cocoa::appkit::{
    NSApplicationActivateIgnoringOtherApps, NSBackingStoreBuffered, NSRunningApplication, NSView,
    NSWindow, NSWindowStyleMask,
};
use cocoa::base::*;
use cocoa::foundation::{NSPoint, NSRect, NSSize};
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

            let content_view = window.contentView();
            let frame = NSView::frame(content_view);

            let view = WindowView::alloc(&inner)?;
            view.initWithFrame_(frame);
            content_view.addSubview_(*view);

            let () = msg_send![*window, setDelegate: *view];

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

    extern "C" fn window_will_close(this: &mut Object, _sel: Sel, _id: id) {
        eprintln!("window_will_close");
        if let Some(this) = Self::get_this(this) {
            // Advise the window of its impending death
            this.inner.lock().unwrap().callbacks.destroy();
        }

        // Release and zero out the inner member
        Self::drop_inner(this);
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
        }

        cls.register()
    }
}
