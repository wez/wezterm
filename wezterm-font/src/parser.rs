use crate::locator::FontDataHandle;
use crate::shaper::GlyphInfo;
use config::FontAttributes;
use std::borrow::Cow;
use std::path::{Path, PathBuf};
use ttf_parser::fonts_in_collection;

#[derive(Debug)]
pub enum MaybeShaped {
    Resolved(GlyphInfo),
    Unresolved { raw: String, slice_start: usize },
}

/// Represents a parsed font
#[derive(Debug)]
pub struct ParsedFont {
    names: Names,
}

#[derive(Debug)]
pub struct Names {
    pub full_name: String,
    pub family: Option<String>,
    pub sub_family: Option<String>,
    pub postscript_name: Option<String>,
}

impl Names {
    pub fn from_ft_face(face: &crate::ftwrap::Face) -> Names {
        let postscript_name = face.postscript_name();
        let family = face.family_name();
        let sub_family = face.style_name();

        let full_name = if sub_family.is_empty() {
            family.to_string()
        } else {
            format!("{} {}", family, sub_family)
        };

        Names {
            full_name,
            family: Some(family),
            sub_family: Some(sub_family),
            postscript_name: Some(postscript_name),
        }
    }
}

impl ParsedFont {
    pub fn from_locator(handle: &FontDataHandle) -> anyhow::Result<Self> {
        let lib = crate::ftwrap::Library::new()?;
        let face = lib.face_from_locator(handle)?;
        Ok(Self {
            names: Names::from_ft_face(&face),
        })
    }

    pub fn names(&self) -> &Names {
        &self.names
    }
}

pub fn font_info_matches(attr: &FontAttributes, names: &Names) -> bool {
    if let Some(fam) = names.family.as_ref() {
        // TODO: correctly match using family and sub-family;
        // this is a pretty rough approximation
        if attr.family == *fam {
            match names.sub_family.as_ref().map(String::as_str) {
                Some("Italic") if attr.italic && !attr.bold => return true,
                Some("Bold") if attr.bold && !attr.italic => return true,
                Some("Bold Italic") if attr.bold && attr.italic => return true,
                Some("Medium") | Some("Regular") | None if !attr.italic && !attr.bold => {
                    return true
                }
                _ => {}
            }
        }
    }
    if attr.family == names.full_name && !attr.bold && !attr.italic {
        true
    } else {
        false
    }
}

/// Given a blob representing a True Type Collection (.ttc) file,
/// and a desired font, enumerate the collection to resolve the index of
/// the font inside that collection that matches it.
/// Even though this is intended to work with a TTC, this also returns
/// the index of a singular TTF file, if it matches.
pub fn resolve_font_from_ttc_data(
    attr: &FontAttributes,
    data: &Cow<'static, [u8]>,
) -> anyhow::Result<Option<usize>> {
    let lib = crate::ftwrap::Library::new()?;
    if let Some(size) = fonts_in_collection(data) {
        for index in 0..size {
            let face = lib.new_face_from_slice(data.clone(), index.into())?;
            let names = Names::from_ft_face(&face);
            if font_info_matches(attr, &names) {
                return Ok(Some(index as usize));
            }
        }
        Ok(None)
    } else {
        let face = lib.new_face_from_slice(data.clone(), 0)?;
        let names = Names::from_ft_face(&face);
        if font_info_matches(attr, &names) {
            Ok(Some(0))
        } else {
            Ok(None)
        }
    }
}

/// In case the user has a broken configuration, or no configuration,
/// we bundle JetBrains Mono and Noto Color Emoji to act as reasonably
/// sane fallback fonts.
/// This function loads those.
pub(crate) fn load_built_in_fonts(
    font_info: &mut Vec<(Names, PathBuf, FontDataHandle)>,
) -> anyhow::Result<()> {
    macro_rules! font {
        ($font:literal) => {
            (include_bytes!($font) as &'static [u8], $font)
        };
    }
    let lib = crate::ftwrap::Library::new()?;
    for (data, name) in &[
        font!("../../assets/fonts/JetBrainsMono-BoldItalic.ttf"),
        font!("../../assets/fonts/JetBrainsMono-Bold.ttf"),
        font!("../../assets/fonts/JetBrainsMono-ExtraBoldItalic.ttf"),
        font!("../../assets/fonts/JetBrainsMono-ExtraBold.ttf"),
        font!("../../assets/fonts/JetBrainsMono-ExtraLightItalic.ttf"),
        font!("../../assets/fonts/JetBrainsMono-ExtraLight.ttf"),
        font!("../../assets/fonts/JetBrainsMono-Italic.ttf"),
        font!("../../assets/fonts/JetBrainsMono-LightItalic.ttf"),
        font!("../../assets/fonts/JetBrainsMono-Light.ttf"),
        font!("../../assets/fonts/JetBrainsMono-MediumItalic.ttf"),
        font!("../../assets/fonts/JetBrainsMono-Medium.ttf"),
        font!("../../assets/fonts/JetBrainsMono-Regular.ttf"),
        font!("../../assets/fonts/JetBrainsMono-ThinItalic.ttf"),
        font!("../../assets/fonts/JetBrainsMono-Thin.ttf"),
        font!("../../assets/fonts/NotoColorEmoji.ttf"),
        font!("../../assets/fonts/PowerlineExtraSymbols.otf"),
        font!("../../assets/fonts/LastResortHE-Regular.ttf"),
    ] {
        let face = lib.new_face_from_slice(Cow::Borrowed(data), 0)?;
        let names = Names::from_ft_face(&face);
        font_info.push((
            names,
            PathBuf::from(name),
            FontDataHandle::Memory {
                data: Cow::Borrowed(data),
                index: 0,
                name: name.to_string(),
                variation: 0,
            },
        ));
    }

    Ok(())
}

pub(crate) fn parse_and_collect_font_info(
    path: &Path,
    font_info: &mut Vec<(Names, PathBuf, FontDataHandle)>,
) -> anyhow::Result<()> {
    let data = Cow::Owned(std::fs::read(path)?);
    let lib = crate::ftwrap::Library::new()?;
    let size = fonts_in_collection(&data).unwrap_or(0);

    fn load_one(
        lib: &crate::ftwrap::Library,
        path: &Path,
        index: u32,
        font_info: &mut Vec<(Names, PathBuf, FontDataHandle)>,
    ) -> anyhow::Result<()> {
        let locator = FontDataHandle::OnDisk {
            path: path.to_path_buf(),
            index,
            variation: 0,
        };

        let face = lib.face_from_locator(&locator)?;
        if let Ok(variations) = face.variations() {
            for (variation, names) in variations.into_iter().enumerate() {
                font_info.push((
                    names,
                    path.to_path_buf(),
                    FontDataHandle::OnDisk {
                        path: path.to_path_buf(),
                        index,
                        variation: variation as u32 + 1,
                    },
                ));
            }
        } else {
            let names = Names::from_ft_face(&face);
            font_info.push((names, path.to_path_buf(), locator));
        }
        Ok(())
    }

    for index in 0..=size {
        if let Err(err) = load_one(&lib, path, index, font_info) {
            log::trace!(
                "error while parsing {} index {}: {}",
                path.display(),
                index,
                err
            );
        }
    }

    Ok(())
}
