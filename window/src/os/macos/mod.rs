#![allow(unexpected_cfgs)] // <https://github.com/SSheldon/rust-objc/issues/125>
use cocoa::base::{id, nil};
use cocoa::foundation::NSString;
use objc::rc::StrongPtr;
use objc::runtime::Object;
use objc::*;

mod app;
pub mod bitmap;
pub mod clipboard;
pub mod connection;
pub mod menu;
pub mod window;

mod keycodes;

pub use self::window::*;
pub use bitmap::*;
pub use connection::*;

/// Convert a rust string to a cocoa string
fn nsstring(s: &str) -> StrongPtr {
    unsafe { StrongPtr::new(NSString::alloc(nil).init_str(s)) }
}

unsafe fn nsstring_to_str<'a>(mut ns: *mut Object) -> &'a str {
    let is_astring: bool = msg_send![ns, isKindOfClass: class!(NSAttributedString)];
    if is_astring {
        ns = msg_send![ns, string];
    }
    let data = NSString::UTF8String(ns as id) as *const u8;
    let len = NSString::len(ns as id);
    let bytes = std::slice::from_raw_parts(data, len);
    std::str::from_utf8_unchecked(bytes)
}
