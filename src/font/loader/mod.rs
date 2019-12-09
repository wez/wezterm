use crate::config::FontAttributes;
use failure::{format_err, Error, Fallible};
use serde_derive::*;
use std::path::PathBuf;
use std::sync::Mutex;

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

#[derive(Debug, Deserialize, Clone, Copy)]
pub enum FontLocatorSelection {
    FontConfig,
    FontLoader,
    FontKit,
}

lazy_static::lazy_static! {
    static ref DEFAULT_LOCATOR: Mutex<FontLocatorSelection> = Mutex::new(Default::default());
}

impl Default for FontLocatorSelection {
    fn default() -> Self {
        if cfg!(all(unix, not(target_os = "macos"))) {
            FontLocatorSelection::FontConfig
        } else {
            FontLocatorSelection::FontLoader
        }
    }
}

impl FontLocatorSelection {
    pub fn set_default(self) {
        let mut def = DEFAULT_LOCATOR.lock().unwrap();
        *def = self;
    }

    pub fn get_default() -> Self {
        let def = DEFAULT_LOCATOR.lock().unwrap();
        *def
    }

    pub fn variants() -> Vec<&'static str> {
        vec!["FontConfig", "FontLoader", "FontKit"]
    }

    pub fn new_locator(self) -> Box<dyn FontLocator> {
        match self {
            Self::FontConfig => {
                #[cfg(all(unix, any(feature = "fontconfig", not(target_os = "macos"))))]
                return Box::new(font_config::FontConfigFontLocator {});
                #[cfg(not(all(unix, any(feature = "fontconfig", not(target_os = "macos")))))]
                panic!("fontconfig not compiled in");
            }
            Self::FontLoader => {
                #[cfg(any(target_os = "macos", windows))]
                return Box::new(font_loader::FontLoaderFontLocator {});
                #[cfg(not(any(target_os = "macos", windows)))]
                panic!("fontloader not compiled in");
            }
            Self::FontKit => {
                #[cfg(any(target_os = "macos", windows))]
                return Box::new(::font_kit::source::SystemSource::new());
                #[cfg(not(any(target_os = "macos", windows)))]
                panic!("fontkit not compiled in");
            }
        }
    }
}

impl std::str::FromStr for FontLocatorSelection {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_ref() {
            "fontconfig" => Ok(Self::FontConfig),
            "fontloader" => Ok(Self::FontLoader),
            "fontkit" => Ok(Self::FontKit),
            _ => Err(format_err!(
                "{} is not a valid FontLocatorSelection variant, possible values are {:?}",
                s,
                Self::variants()
            )),
        }
    }
}
