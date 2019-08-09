use super::nsstring;
use crate::WindowCallbacks;
use cocoa::appkit::{
    NSApplicationActivateIgnoringOtherApps, NSBackingStoreBuffered, NSRunningApplication, NSWindow,
    NSWindowStyleMask,
};
use cocoa::base::*;
use cocoa::foundation::{NSAutoreleasePool, NSPoint, NSRect, NSSize};
use failure::Fallible;
use objc::*;

pub struct Window {
    window: id,
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

            let window = NSWindow::alloc(nil).initWithContentRect_styleMask_backing_defer_(
                rect,
                style_mask,
                NSBackingStoreBuffered,
                NO,
            );

            window.autorelease();
            window.cascadeTopLeftFromPoint_(NSPoint::new(20.0, 20.0));
            window.setTitle_(nsstring(&name));
            window.setAcceptsMouseMovedEvents_(YES);

            Ok(Self { window })
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
