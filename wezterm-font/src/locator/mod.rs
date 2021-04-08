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

#[derive(Clone)]
pub enum FontDataSource {
    OnDisk(PathBuf),
    Memory {
        name: String,
        data: Cow<'static, [u8]>,
    },
}

impl FontDataSource {
    pub fn name_or_path_str(&self) -> Cow<str> {
        match self {
            Self::OnDisk(path) => path.to_string_lossy(),
            Self::Memory { name, .. } => Cow::Borrowed(name),
        }
    }

    pub fn load_data<'a>(&'a self) -> anyhow::Result<Cow<'a, [u8]>> {
        match self {
            Self::OnDisk(path) => {
                let data = std::fs::read(path)?;
                Ok(Cow::Owned(data))
            }
            Self::Memory { data, .. } => Ok(data.clone()),
        }
    }
}

impl Eq for FontDataSource {}

impl PartialEq for FontDataSource {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::OnDisk(path_a), Self::OnDisk(path_b)) => path_a == path_b,
            (Self::Memory { name: name_a, .. }, Self::Memory { name: name_b, .. }) => {
                name_a == name_b
            }
            _ => false,
        }
    }
}

impl Ord for FontDataSource {
    fn cmp(&self, other: &Self) -> Ordering {
        let a = self.name_or_path_str();
        let b = other.name_or_path_str();
        a.cmp(&b)
    }
}

impl PartialOrd for FontDataSource {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl std::fmt::Debug for FontDataSource {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        match self {
            Self::OnDisk(path) => fmt.debug_struct("OnDisk").field("path", &path).finish(),
            Self::Memory { data, name } => fmt
                .debug_struct("Memory")
                .field("name", &name)
                .field("data_len", &data.len())
                .finish(),
        }
    }
}

/// Represents the data behind a font.
/// This may be a font file that we can read off disk,
/// or some data that resides in memory.
/// The `index` parameter is the index into a font
/// collection if the data represents a collection of
/// fonts.
#[derive(Clone, Debug, PartialEq, Eq, Ord, PartialOrd)]
pub struct FontDataHandle {
    pub source: FontDataSource,
    pub index: u32,
    pub variation: u32,
}

impl FontDataHandle {
    pub fn name_or_path_str(&self) -> Cow<str> {
        self.source.name_or_path_str()
    }

    pub fn index(&self) -> u32 {
        self.index
    }

    pub fn set_index(&mut self, idx: u32) {
        self.index = idx;
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
