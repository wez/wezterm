#[cfg(not(feature = "std"))]
extern crate alloc;

pub use alloc::borrow::ToOwned;
pub use alloc::boxed::Box;
pub use alloc::collections::BTreeMap;
pub use alloc::string::{String, ToString};
pub use alloc::vec::Vec;

#[cfg(not(feature = "std"))]
pub use alloc::collections::BTreeMap as HashMap;
#[cfg(feature = "std")]
pub use std::collections::HashMap;
