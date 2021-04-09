use crate::locator::{FontDataHandle, FontDataSource};
use crate::shaper::GlyphInfo;
use config::FontAttributes;
pub use config::{FontStretch, FontWeight};
use std::borrow::Cow;

#[derive(Debug)]
pub enum MaybeShaped {
    Resolved(GlyphInfo),
    Unresolved { raw: String, slice_start: usize },
}

/// Represents a parsed font
#[derive(Debug, PartialEq, Eq, Clone, PartialOrd, Ord)]
pub struct ParsedFont {
    names: Names,
    weight: FontWeight,
    stretch: FontStretch,
    italic: bool,
    pub handle: FontDataHandle,
}

#[derive(Debug, PartialEq, Eq, Clone, PartialOrd, Ord)]
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
        Self::from_face(&face, handle.clone())
    }

    pub fn from_face(face: &crate::ftwrap::Face, handle: FontDataHandle) -> anyhow::Result<Self> {
        let italic = face.italic();
        let (weight, width) = face.weight_and_width();
        let weight = FontWeight::from_opentype_weight(weight);
        let stretch = FontStretch::from_opentype_stretch(width);

        Ok(Self {
            names: Names::from_ft_face(&face),
            weight,
            stretch,
            italic,
            handle,
        })
    }

    pub fn names(&self) -> &Names {
        &self.names
    }

    pub fn weight(&self) -> FontWeight {
        self.weight
    }

    pub fn stretch(&self) -> FontStretch {
        self.stretch
    }

    pub fn italic(&self) -> bool {
        self.italic
    }

    pub fn matches_attributes(&self, attr: &FontAttributes) -> FontMatch {
        if let Some(fam) = self.names.family.as_ref() {
            if attr.family == *fam {
                if attr.stretch == self.stretch {
                    let wanted_weight = attr.weight.to_opentype_weight();
                    let weight = self.weight.to_opentype_weight();

                    if weight >= wanted_weight {
                        if attr.italic == self.italic {
                            return FontMatch::Weight(weight - wanted_weight);
                        }
                    }

                    if attr.family == self.names.full_name {
                        return FontMatch::FullName;
                    }
                }
            }
        }

        if attr.family == self.names.full_name {
            FontMatch::FullName
        } else if let Some(ps) = self.names.postscript_name.as_ref() {
            if attr.family == *ps {
                FontMatch::FullName
            } else {
                FontMatch::NoMatch
            }
        } else {
            FontMatch::NoMatch
        }
    }

    pub fn rank_matches(attr: &FontAttributes, fonts: Vec<Self>) -> Vec<Self> {
        let mut candidates = vec![];
        for p in fonts {
            let res = p.matches_attributes(attr);
            if res != FontMatch::NoMatch {
                candidates.push((res, p));
            }
        }
        candidates.sort_by(|a, b| a.0.cmp(&b.0));
        candidates.into_iter().map(|(_, p)| p).collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Ord, PartialOrd)]
pub enum FontMatch {
    Weight(u16),
    FullName,
    NoMatch,
}

/// In case the user has a broken configuration, or no configuration,
/// we bundle JetBrains Mono and Noto Color Emoji to act as reasonably
/// sane fallback fonts.
/// This function loads those.
pub(crate) fn load_built_in_fonts(font_info: &mut Vec<ParsedFont>) -> anyhow::Result<()> {
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
        let locator = FontDataHandle {
            source: FontDataSource::Memory {
                data: Cow::Borrowed(data),
                name: name.to_string(),
            },
            index: 0,
            variation: 0,
        };
        let face = lib.face_from_locator(&locator)?;
        let parsed = ParsedFont::from_face(&face, locator)?;
        font_info.push(parsed);
    }

    Ok(())
}

pub fn rank_matching_fonts(
    source: &FontDataSource,
    font_attr: &FontAttributes,
) -> anyhow::Result<Vec<ParsedFont>> {
    let mut font_info = vec![];
    parse_and_collect_font_info(source, &mut font_info)?;
    Ok(ParsedFont::rank_matches(font_attr, font_info))
}

pub(crate) fn parse_and_collect_font_info(
    source: &FontDataSource,
    font_info: &mut Vec<ParsedFont>,
) -> anyhow::Result<()> {
    let lib = crate::ftwrap::Library::new()?;
    let num_faces = lib.query_num_faces(&source)?;

    fn load_one(
        lib: &crate::ftwrap::Library,
        source: &FontDataSource,
        index: u32,
        font_info: &mut Vec<ParsedFont>,
    ) -> anyhow::Result<()> {
        let locator = FontDataHandle {
            source: source.clone(),
            index,
            variation: 0,
        };

        let face = lib.face_from_locator(&locator)?;
        if let Ok(variations) = face.variations() {
            for parsed in variations {
                font_info.push(parsed);
            }
        } else {
            let parsed = ParsedFont::from_locator(&locator)?;
            font_info.push(parsed);
        }
        Ok(())
    }

    for index in 0..num_faces {
        if let Err(err) = load_one(&lib, &source, index, font_info) {
            log::trace!("error while parsing {:?} index {}: {}", source, index, err);
        }
    }

    Ok(())
}
