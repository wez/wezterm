use crate::fcwrap;
use crate::locator::{FontDataHandle, FontLocator};
use config::FontAttributes;
use fcwrap::Pattern as FontPattern;
use std::collections::HashSet;
use std::convert::TryInto;

/// A FontLocator implemented using the system font loading
/// functions provided by font-config
pub struct FontConfigFontLocator {}

impl FontLocator for FontConfigFontLocator {
    fn load_fonts(
        &self,
        fonts_selection: &[FontAttributes],
        loaded: &mut HashSet<FontAttributes>,
    ) -> anyhow::Result<Vec<FontDataHandle>> {
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

            for pat in font_list.iter() {
                pattern.render_prepare(&pat)?;
                let file = pat.get_file()?;

                let handle = FontDataHandle::OnDisk {
                    path: file.into(),
                    index: pat.get_integer("index")?.try_into()?,
                };

                // fontconfig will give us a boatload of random fallbacks.
                // so we need to parse the returned font
                // here to see if we got what we asked for.
                if let Ok(parsed) = crate::parser::ParsedFont::from_locator(&handle) {
                    if crate::parser::font_info_matches(attr, parsed.names()) {
                        fonts.push(handle);
                        loaded.insert(attr.clone());
                    }
                }
            }
        }

        fonts.append(&mut fallback);

        Ok(fonts)
    }
}
