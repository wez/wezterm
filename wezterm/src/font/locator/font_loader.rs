use crate::font::locator::{FontDataHandle, FontLocator};
use ::font_loader::system_fonts;
use config::FontAttributes;
use std::collections::HashSet;

/// A FontLocator implemented using the system font loading
/// functions provided by the font-loader crate.
pub struct FontLoaderFontLocator {}

impl FontLocator for FontLoaderFontLocator {
    fn load_fonts(
        &self,
        fonts_selection: &[FontAttributes],
        loaded: &mut HashSet<FontAttributes>,
    ) -> anyhow::Result<Vec<FontDataHandle>> {
        let mut fonts = Vec::new();
        for font_attr in fonts_selection {
            if cfg!(windows) && font_attr.family.len() > 31 {
                // Avoid a super painful panic in the upstream library:
                // https://github.com/MSleepyPanda/rust-font-loader/blob/2b264974fe080955d341ce8a163035bdce24ff2f/src/win32.rs#L87
                log::error!(
                    "font-loader would panic for font family `{}`",
                    font_attr.family
                );
                continue;
            }
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
                    name: font_attr.family.clone(),
                };
                fonts.push(handle);
                loaded.insert(font_attr.clone());
            }
        }
        Ok(fonts)
    }
}
