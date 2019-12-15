use crate::config::FontAttributes;
use crate::font::locator::{FontDataHandle, FontLocator};
use ::font_loader::system_fonts;

/// A FontLocator implemented using the system font loading
/// functions provided by the font-loader crate.
pub struct FontLoaderFontLocator {}

impl FontLocator for FontLoaderFontLocator {
    fn load_fonts(
        &self,
        fonts_selection: &[FontAttributes],
    ) -> anyhow::Result<Vec<FontDataHandle>> {
        let mut fonts = Vec::new();
        for font_attr in fonts_selection {
            let mut font_props = system_fonts::FontPropertyBuilder::new()
                .family(&font_attr.family)
                .monospace();
            font_props = if font_attr.bold {
                font_props.bold()
            } else {
                font_props
            };
            font_props = if font_attr.italic {
                font_props.italic()
            } else {
                font_props
            };
            let font_props = font_props.build();

            if let Some((data, index)) = system_fonts::get(&font_props) {
                let handle = FontDataHandle::Memory {
                    data,
                    index: index as u32,
                };
                fonts.push(handle);
            }
        }
        Ok(fonts)
    }
}
