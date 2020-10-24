use crate::locator::{FontDataHandle, FontLocator};
use ::font_kit::family_name::FamilyName;
use ::font_kit::handle::Handle;
use ::font_kit::properties::Properties;
use ::font_kit::source::Source;
use config::FontAttributes;
use std::collections::HashSet;

/// A FontLocator implemented using the font loading
/// functions provided by Source's from font-kit crate.
impl<S> FontLocator for S
where
    S: Source,
{
    fn load_fonts(
        &self,
        fonts_selection: &[FontAttributes],
        loaded: &mut HashSet<FontAttributes>,
    ) -> anyhow::Result<Vec<FontDataHandle>> {
        let mut handles = vec![];

        for font in fonts_selection {
            let mut props = Properties::new();
            if font.bold {
                props.weight(font_kit::properties::Weight::BOLD);
            }
            if font.italic {
                props.style(font_kit::properties::Style::Italic);
            }
            let family = FamilyName::Title(font.family.clone());
            match self.select_best_match(&[family.clone()], &props) {
                Ok(Handle::Path { path, font_index }) => handles.push(FontDataHandle::OnDisk {
                    path,
                    index: font_index,
                }),
                Ok(Handle::Memory { bytes, font_index }) => handles.push(FontDataHandle::Memory {
                    data: bytes.to_vec(),
                    index: font_index,
                    name: font.family.clone(),
                }),
                Err(_) => continue,
            }
            loaded.insert(font.clone());
        }

        Ok(handles)
    }
}
