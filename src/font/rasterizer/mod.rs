use crate::font::locator::FontDataHandle;
use crate::font::units::*;
use anyhow::{anyhow, bail, Error};
use serde_derive::*;
use std::sync::Mutex;

pub mod freetype;

/// A bitmap representation of a glyph.
/// The data is stored as pre-multiplied RGBA 32bpp.
pub struct RasterizedGlyph {
    pub data: Vec<u8>,
    pub height: usize,
    pub width: usize,
    pub bearing_x: PixelLength,
    pub bearing_y: PixelLength,
    pub has_color: bool,
}

/// Rasterizes the specified glyph index in the associated font
/// and returns the generated bitmap
pub trait FontRasterizer {
    fn rasterize_glyph(
        &self,
        glyph_pos: u32,
        size: f64,
        dpi: u32,
    ) -> anyhow::Result<RasterizedGlyph>;
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

    pub fn new_rasterizer(
        self,
        handle: &FontDataHandle,
    ) -> anyhow::Result<Box<dyn FontRasterizer>> {
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
            _ => Err(anyhow!(
                "{} is not a valid FontRasterizerSelection variant, possible values are {:?}",
                s,
                Self::variants()
            )),
        }
    }
}
