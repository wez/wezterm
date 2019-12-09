use crate::config::FontAttributes;
use crate::font::loader::{FontDataHandle, FontLocator};
use failure::Fallible;

/// A FontLocator implemented using the system font loading
/// functions provided by font-config
pub struct FontConfigFontLocator {}

impl FontLocator for FontLoaderFontLocator {
    fn load_fonts(&self, fonts_selection: &[FontAttributes]) -> Fallible<Vec<FontDataHandle>> {
        let mut fonts = vec![];
        let mut fallback = vec![];

        for attr in style.font_with_fallback() {
            let mut pattern = FontPattern::new()?;
            pattern.family(&attr.family)?;
            if attr.bold {
                pattern.add_integer("weight", 200)?;
            }
            if attr.italic {
                pattern.add_integer("slant", 100)?;
            }
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
                    index: 0, // FIXME: extract this from pat!
                };

                // When it comes to handling fallback, we prefer our
                // user specified set of names so we take those first.
                // The additional items in this loop are fallback fonts
                // suggested by fontconfig and are lower precedence
                if idx == 0 {
                    self.fonts.push(handle);
                } else {
                    self.fallback.push(handle);
                }
            }
        }

        fonts.extend_from_slice(&mut fallback);

        Ok(fonts)
    }
}
