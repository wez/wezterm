use crate::config::FontAttributes;
use failure::Fallible;
use std::path::PathBuf;

#[cfg(all(unix, any(feature = "fontconfig", not(target_os = "macos"))))]
pub mod font_config;
#[cfg(any(target_os = "macos", windows))]
pub mod font_kit;
#[cfg(any(target_os = "macos", windows))]
pub mod font_loader;

/// Represents the data behind a font.
/// This may be a font file that we can read off disk,
/// or some data that resides in memory.
/// The `index` parameter is the index into a font
/// collection if the data represents a collection of
/// fonts.
pub enum FontDataHandle {
    OnDisk { path: PathBuf, index: u32 },
    Memory { data: Vec<u8>, index: u32 },
}

pub trait FontLocator {
    /// Given a font selection, return the list of successfully loadable
    /// FontDataHandle's that correspond to it
    fn load_fonts(&self, fonts_selection: &[FontAttributes]) -> Fallible<Vec<FontDataHandle>>;
}
