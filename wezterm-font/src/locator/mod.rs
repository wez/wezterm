use config::FontAttributes;
use std::collections::HashSet;
use std::path::PathBuf;

pub mod enum_font_families;
#[cfg(all(unix, not(target_os = "macos")))]
pub mod font_config;
#[cfg(target_os = "macos")]
pub mod font_loader;

/// Represents the data behind a font.
/// This may be a font file that we can read off disk,
/// or some data that resides in memory.
/// The `index` parameter is the index into a font
/// collection if the data represents a collection of
/// fonts.
#[derive(Clone)]
pub enum FontDataHandle {
    OnDisk {
        path: PathBuf,
        index: u32,
    },
    #[allow(dead_code)]
    Memory {
        name: String,
        data: Vec<u8>,
        index: u32,
    },
}

impl std::fmt::Debug for FontDataHandle {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        match self {
            Self::OnDisk { path, index } => fmt
                .debug_struct("OnDisk")
                .field("path", &path)
                .field("index", &index)
                .finish(),
            Self::Memory { data, index, name } => fmt
                .debug_struct("Memory")
                .field("name", &name)
                .field("data_len", &data.len())
                .field("index", &index)
                .finish(),
        }
    }
}

pub trait FontLocator {
    /// Given a font selection, return the list of successfully loadable
    /// FontDataHandle's that correspond to it
    fn load_fonts(
        &self,
        fonts_selection: &[FontAttributes],
        loaded: &mut HashSet<FontAttributes>,
    ) -> anyhow::Result<Vec<FontDataHandle>>;
}

pub fn new_locator(locator: FontLocatorSelection) -> Box<dyn FontLocator> {
    match locator {
        FontLocatorSelection::FontConfig => {
            #[cfg(all(unix, not(target_os = "macos")))]
            return Box::new(font_config::FontConfigFontLocator {});
            #[cfg(not(all(unix, not(target_os = "macos"))))]
            panic!("fontconfig not compiled in");
        }
        FontLocatorSelection::FontLoader => {
            #[cfg(target_os = "macos")]
            return Box::new(font_loader::FontLoaderFontLocator {});
            #[cfg(not(target_os = "macos"))]
            panic!("fontloader not compiled in");
        }
        FontLocatorSelection::EnumFontFamilies => {
            #[cfg(windows)]
            return Box::new(enum_font_families::EnumFontFamiliesFontLocator {});
            #[cfg(not(windows))]
            panic!("EnumFontFamilies not compiled in");
        }
        FontLocatorSelection::ConfigDirsOnly => Box::new(NopSystemSource {}),
    }
}

struct NopSystemSource {}

pub use config::FontLocatorSelection;

impl FontLocator for NopSystemSource {
    fn load_fonts(
        &self,
        _fonts_selection: &[FontAttributes],
        _loaded: &mut HashSet<FontAttributes>,
    ) -> anyhow::Result<Vec<FontDataHandle>> {
        Ok(vec![])
    }
}
