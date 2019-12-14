use crate::config::FontAttributes;
use crate::font::fcwrap;
use crate::font::locator::{FontDataHandle, FontLocator};
use failure::Fallible;
use fcwrap::Pattern as FontPattern;
use std::convert::TryInto;

/// A FontLocator implemented using the system font loading
/// functions provided by font-config
pub struct FontConfigFontLocator {}

impl FontLocator for FontConfigFontLocator {
    fn load_fonts(&self, fonts_selection: &[FontAttributes]) -> Fallible<Vec<FontDataHandle>> {
        let mut fonts = vec![];
        let mut fallback = vec![];

        for attr in fonts_selection {
            let mut pattern = FontPattern::new()?;
            pattern.family(&attr.family)?;
            pattern.add_integer("weight", if attr.bold { 200 } else { 80 })?;
            pattern.add_integer("slant", if attr.italic { 100 } else { 0 })?;
            /*
            pattern.add_double("size", config.font_size * font_scale)?;
            pattern.add_double("dpi", config.dpi)?;
            */
            pattern.monospace()?;
            pattern.config_substitute(fcwrap::MatchKind::Pattern)?;
            pattern.default_substitute();
            // and obtain the selection with the best preference
            // at index 0.
            let font_list = pattern.sort(true)?;

            for (idx, pat) in font_list.iter().enumerate() {
                pattern.render_prepare(&pat)?;
                let file = pat.get_file()?;

                let handle = FontDataHandle::OnDisk {
                    path: file.into(),
                    index: pat.get_integer("index")?.try_into()?,
                };

                // When it comes to handling fallback, we prefer our
                // user specified set of names so we take those first.
                // The additional items in this loop are fallback fonts
                // suggested by fontconfig and are lower precedence
                if idx == 0 {
                    fonts.push(handle);
                } else {
                    fallback.push(handle);
                }
            }
        }

        fonts.append(&mut fallback);

        Ok(fonts)
    }
}
