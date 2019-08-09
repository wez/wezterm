use cocoa::base::nil;
use cocoa::foundation::NSString;

pub mod connection;
pub mod window;

pub use connection::*;
pub use window::*;

/// Convert a rust string to a cocoa string
fn nsstring(s: &str) -> cocoa::base::id {
    // ARC will free this at the appropriate time
    unsafe { NSString::alloc(nil).init_str(s) }
}
