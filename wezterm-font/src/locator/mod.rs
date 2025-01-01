use crate::parser::ParsedFont;
use config::FontAttributes;
use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::HashSet;
use std::fmt::Display;
use std::path::PathBuf;
use std::sync::Arc;

pub mod core_text;
#[cfg(all(unix, not(target_os = "macos")))]
pub mod font_config;
pub mod gdi;

#[derive(Clone, Debug, PartialEq, Eq, Ord, PartialOrd, Hash)]
pub enum FontOrigin {
    FontConfig,
    FontConfigMatch(String),
    CoreText,
    DirectWrite,
    Gdi,
    FontDirs,
    BuiltIn,
}

// derived impl would just use the inner string instead of
// 'FontConfigMatch("..")', so use Debug
impl Display for FontOrigin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Clone, Hash)]
pub enum FontDataSource {
    OnDisk(PathBuf),
    BuiltIn {
        name: &'static str,
        data: &'static [u8],
    },
    Memory {
        name: String,
        data: Arc<Box<[u8]>>,
    },
}

impl FontDataSource {
    pub fn name_or_path_str(&self) -> Cow<str> {
        match self {
            Self::OnDisk(path) => path.to_string_lossy(),
            Self::BuiltIn { name, .. } => Cow::Borrowed(name),
            Self::Memory { name, .. } => Cow::Borrowed(name),
        }
    }

    pub fn path_str(&self) -> Option<Cow<str>> {
        match self {
            Self::OnDisk(path) => Some(path.to_string_lossy()),
            Self::BuiltIn { .. } => None,
            Self::Memory { .. } => None,
        }
    }

    pub fn load_data<'a>(&'a self) -> anyhow::Result<Cow<'a, [u8]>> {
        match self {
            Self::OnDisk(path) => {
                let data = std::fs::read(path)?;
                Ok(Cow::Owned(data))
            }
            Self::BuiltIn { data, .. } => Ok(Cow::Borrowed(data)),
            Self::Memory { data, .. } => Ok(Cow::Borrowed(&*data)),
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
            (Self::BuiltIn { name: name_a, .. }, Self::BuiltIn { name: name_b, .. }) => {
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
            Self::BuiltIn { name, .. } => fmt.debug_struct("BuiltIn").field("name", &name).finish(),
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
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FontDataHandle {
    pub source: FontDataSource,
    pub index: u32,
    pub variation: u32,
    pub origin: FontOrigin,
    pub coverage: Option<rangeset::RangeSet<u32>>,
}

impl std::hash::Hash for FontDataHandle {
    fn hash<H>(&self, hasher: &mut H)
    where
        H: std::hash::Hasher,
    {
        (&self.source, self.index, self.variation, &self.origin).hash(hasher)
    }
}

impl PartialOrd for FontDataHandle {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        (&self.source, self.index, self.variation, &self.origin).partial_cmp(&(
            &other.source,
            other.index,
            other.variation,
            &other.origin,
        ))
    }
}

impl Ord for FontDataHandle {
    fn cmp(&self, other: &Self) -> Ordering {
        (&self.source, self.index, self.variation, &self.origin).cmp(&(
            &other.source,
            other.index,
            other.variation,
            &other.origin,
        ))
    }
}

impl FontDataHandle {
    pub fn name_or_path_str(&self) -> Cow<str> {
        self.source.name_or_path_str()
    }

    pub fn path_str(&self) -> Option<Cow<str>> {
        self.source.path_str()
    }

    pub fn index(&self) -> u32 {
        self.index
    }

    pub fn set_index(&mut self, idx: u32) {
        self.index = idx;
    }

    pub fn diagnostic_string(&self) -> String {
        let source = match &self.source {
            FontDataSource::OnDisk(path) => format!("{}", path.display()),
            FontDataSource::BuiltIn { .. } => "<built-in>".to_string(),
            FontDataSource::Memory { .. } => "<imported to RAM>".to_string(),
        };

        if self.index == 0 && self.variation == 0 {
            format!("{}, {}", source, self.origin)
        } else {
            format!(
                "{} index={} variation={}, {}",
                source, self.index, self.variation, self.origin
            )
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
        pixel_size: u16,
    ) -> anyhow::Result<Vec<ParsedFont>>;

    fn enumerate_all_fonts(&self) -> anyhow::Result<Vec<ParsedFont>> {
        Ok(vec![])
    }

    fn locate_fallback_for_codepoints(
        &self,
        codepoints: &[char],
    ) -> anyhow::Result<Vec<ParsedFont>>;
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
        _pixel_size: u16,
    ) -> anyhow::Result<Vec<ParsedFont>> {
        Ok(vec![])
    }

    fn enumerate_all_fonts(&self) -> anyhow::Result<Vec<ParsedFont>> {
        Ok(vec![])
    }

    fn locate_fallback_for_codepoints(
        &self,
        _codepoints: &[char],
    ) -> anyhow::Result<Vec<ParsedFont>> {
        Ok(vec![])
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_memory_datasource() {
        // This is currently only used on Windows, so make sure
        // that we have compiler coverage of constructing it on
        // other systems
        let data = b"hello".to_vec();
        let source = FontDataSource::Memory {
            data: Arc::new(data.into_boxed_slice()),
            name: "hello!".to_string(),
        };
        eprintln!("{:?}", source);
    }
}
