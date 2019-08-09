use cocoa::base::nil;
use cocoa::foundation::NSString;
use objc::rc::StrongPtr;

pub mod connection;
pub mod window;

pub use connection::*;
pub use window::*;

/// Convert a rust string to a cocoa string
fn nsstring(s: &str) -> StrongPtr {
    unsafe { StrongPtr::new(NSString::alloc(nil).init_str(s)) }
}
