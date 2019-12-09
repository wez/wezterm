use crate::font::loader::FontDataHandle;
use crate::font::system::RasterizedGlyph;
use failure::{bail, format_err, Error, Fallible};
use serde_derive::*;
use std::sync::Mutex;

pub mod freetype;

/// Rasterizes the specified glyph index in the associated font
/// and returns the generated bitmap
pub trait FontRasterizer {
    fn rasterize_glyph(&self, glyph_pos: u32, size: f64, dpi: u32) -> Fallible<RasterizedGlyph>;
}

#[derive(Debug, Deserialize, Clone, Copy)]
pub enum FontRasterizerSelection {
    FreeType,
    FontKit,
}

lazy_static::lazy_static! {
    static ref DEFAULT_RASTER: Mutex<FontRasterizerSelection> = Mutex::new(Default::default());
}

impl Default for FontRasterizerSelection {
    fn default() -> Self {
        FontRasterizerSelection::FreeType
    }
}

impl FontRasterizerSelection {
    pub fn set_default(self) {
        let mut def = DEFAULT_RASTER.lock().unwrap();
        *def = self;
    }

    pub fn get_default() -> Self {
        let def = DEFAULT_RASTER.lock().unwrap();
        *def
    }

    pub fn variants() -> Vec<&'static str> {
        vec!["FreeType", "FontKit"]
    }

    pub fn new_rasterizer(self, handle: &FontDataHandle) -> Fallible<Box<dyn FontRasterizer>> {
        match self {
            Self::FreeType => Ok(Box::new(freetype::FreeTypeRasterizer::from_locator(
                handle,
            )?)),
            Self::FontKit => bail!("FontKit rasterizer not implemented yet"),
        }
    }
}

impl std::str::FromStr for FontRasterizerSelection {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_ref() {
            "freetype" => Ok(Self::FreeType),
            "fontkit" => Ok(Self::FontKit),
            _ => Err(format_err!(
                "{} is not a valid FontRasterizerSelection variant, possible values are {:?}",
                s,
                Self::variants()
            )),
        }
    }
}
