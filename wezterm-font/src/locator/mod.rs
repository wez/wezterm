use config::FontAttributes;
use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

pub mod core_text;
#[cfg(all(unix, not(target_os = "macos")))]
pub mod font_config;
pub mod gdi;

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
    Memory {
        name: String,
        data: std::borrow::Cow<'static, [u8]>,
        index: u32,
    },
}

impl FontDataHandle {
    fn name_or_path_str(&self) -> Cow<str> {
        match self {
            Self::OnDisk { path, .. } => path.to_string_lossy(),
            Self::Memory { name, .. } => Cow::Borrowed(name),
        }
    }
    fn index(&self) -> u32 {
        match self {
            Self::OnDisk { index, .. } => *index,
            Self::Memory { index, .. } => *index,
        }
    }
}

impl Eq for FontDataHandle {}

impl PartialEq for FontDataHandle {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::OnDisk {
                    path: path_a,
                    index: index_a,
                },
                Self::OnDisk {
                    path: path_b,
                    index: index_b,
                },
            ) => path_a == path_b && index_a == index_b,
            (
                Self::Memory {
                    name: name_a,
                    index: index_a,
                    ..
                },
                Self::Memory {
                    name: name_b,
                    index: index_b,
                    ..
                },
            ) => name_a == name_b && index_a == index_b,
            _ => false,
        }
    }
}

impl Ord for FontDataHandle {
    fn cmp(&self, other: &Self) -> Ordering {
        let a = (self.name_or_path_str(), self.index());
        let b = (other.name_or_path_str(), other.index());
        a.cmp(&b)
    }
}

impl PartialOrd for FontDataHandle {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
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

    fn locate_fallback_for_codepoints(
        &self,
        codepoints: &[char],
    ) -> anyhow::Result<Vec<FontDataHandle>>;
}

pub fn new_locator(locator: FontLocatorSelection) -> Arc<dyn FontLocator + Send + Sync> {
    match locator {
        FontLocatorSelection::FontConfig => {
            #[cfg(all(unix, not(target_os = "macos")))]
            return Arc::new(font_config::FontConfigFontLocator {});
            #[cfg(not(all(unix, not(target_os = "macos"))))]
            panic!("fontconfig not compiled in");
        }
        FontLocatorSelection::CoreText => {
            #[cfg(target_os = "macos")]
            return Arc::new(core_text::CoreTextFontLocator {});
            #[cfg(not(target_os = "macos"))]
            panic!("CoreText not compiled in");
        }
        FontLocatorSelection::Gdi => {
            #[cfg(windows)]
            return Arc::new(gdi::GdiFontLocator {});
            #[cfg(not(windows))]
            panic!("Gdi not compiled in");
        }
        FontLocatorSelection::ConfigDirsOnly => Arc::new(NopSystemSource {}),
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

    fn locate_fallback_for_codepoints(
        &self,
        _codepoints: &[char],
    ) -> anyhow::Result<Vec<FontDataHandle>> {
        Ok(vec![])
    }
}
