use crate::font::loader::FontDataHandle;
use crate::font::system::GlyphInfo;
use failure::{format_err, Error, Fallible};
use serde_derive::*;
use std::sync::Mutex;

pub mod harfbuzz;

pub trait FontShaper {
    /// Shape text and return a vector of GlyphInfo
    fn shape(&self, text: &str, size: f64, dpi: u32) -> Fallible<Vec<GlyphInfo>>;
}

#[derive(Debug, Deserialize, Clone, Copy)]
pub enum FontShaperSelection {
    Harfbuzz,
}

lazy_static::lazy_static! {
    static ref DEFAULT_SHAPER: Mutex<FontShaperSelection> = Mutex::new(Default::default());
}

impl Default for FontShaperSelection {
    fn default() -> Self {
        FontShaperSelection::Harfbuzz
    }
}

impl FontShaperSelection {
    pub fn set_default(self) {
        let mut def = DEFAULT_SHAPER.lock().unwrap();
        *def = self;
    }

    pub fn get_default() -> Self {
        let def = DEFAULT_SHAPER.lock().unwrap();
        *def
    }

    pub fn variants() -> Vec<&'static str> {
        vec!["Harfbuzz"]
    }

    pub fn new_shaper(self, handles: &[FontDataHandle]) -> Fallible<Box<dyn FontShaper>> {
        match self {
            Self::Harfbuzz => Ok(Box::new(harfbuzz::HarfbuzzShaper::new(handles)?)),
        }
    }
}

impl std::str::FromStr for FontShaperSelection {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_ref() {
            "harfbuzz" => Ok(Self::Harfbuzz),
            _ => Err(format_err!(
                "{} is not a valid FontShaperSelection variant, possible values are {:?}",
                s,
                Self::variants()
            )),
        }
    }
}
