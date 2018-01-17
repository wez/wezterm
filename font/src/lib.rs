#[macro_use]
extern crate failure;

extern crate harfbuzz_sys;
#[cfg(not(target_os = "macos"))]
extern crate fontconfig; // from servo-fontconfig
#[cfg(not(target_os = "macos"))]
extern crate freetype;

use failure::Error;

#[cfg(not(target_os = "macos"))]
pub mod ft;
#[cfg(not(target_os = "macos"))]
pub use ft::FTEngine as Engine;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FontDescription {
    name: String,
}

/// A user provided font description that can be used
/// to lookup a font
impl FontDescription {
    pub fn new<S>(name: S) -> FontDescription
    where
        S: Into<String>,
    {
        FontDescription { name: name.into() }
    }
}

trait FontEngine {
    fn new() -> Result<Self, Error>
    where
        Self: Sized;
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
