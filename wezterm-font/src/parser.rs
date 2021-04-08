use crate::locator::FontDataHandle;
use crate::shaper::GlyphInfo;
use config::FontAttributes;
use std::borrow::Cow;
use std::path::Path;

#[derive(Debug)]
pub enum MaybeShaped {
    Resolved(GlyphInfo),
    Unresolved { raw: String, slice_start: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontWidth {
    UltraCondensed,
    ExtraCondensed,
    Condensed,
    SemiCondensed,
    Normal,
    SemiExpanded,
    Expanded,
    ExtraExpanded,
    UltraExpanded,
}

impl FontWidth {
    pub fn from_opentype_width(w: u16) -> Self {
        match w {
            1 => Self::UltraCondensed,
            2 => Self::ExtraCondensed,
            3 => Self::Condensed,
            4 => Self::SemiCondensed,
            5 => Self::Normal,
            6 => Self::SemiExpanded,
            7 => Self::Expanded,
            8 => Self::ExtraExpanded,
            9 => Self::UltraExpanded,
            _ if w < 1 => Self::UltraCondensed,
            _ => Self::UltraExpanded,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontWeight {
    Thin,
    ExtraLight,
    Light,
    DemiLight,
    Book,
    Regular,
    Medium,
    DemiBold,
    Bold,
    ExtraBold,
    Black,
    ExtraBlack,
}

impl FontWeight {
    pub fn from_opentype_weight(w: u16) -> Self {
        if w >= 1000 {
            Self::ExtraBlack
        } else if w >= 900 {
            Self::Black
        } else if w >= 800 {
            Self::ExtraBold
        } else if w >= 700 {
            Self::Bold
        } else if w >= 600 {
            Self::DemiBold
        } else if w >= 500 {
            Self::Medium
        } else if w >= 400 {
            Self::Regular
        } else if w >= 380 {
            Self::Book
        } else if w >= 350 {
            Self::DemiLight
        } else if w >= 300 {
            Self::Light
        } else if w >= 200 {
            Self::ExtraLight
        } else {
            Self::Thin
        }
    }

    pub fn to_opentype_weight(self) -> u16 {
        match self {
            Self::Thin => 100,
            Self::ExtraLight => 200,
            Self::Light => 300,
            Self::DemiLight => 350,
            Self::Book => 380,
            Self::Regular => 400,
            Self::Medium => 500,
            Self::DemiBold => 600,
            Self::Bold => 700,
            Self::ExtraBold => 800,
            Self::Black => 900,
            Self::ExtraBlack => 1000,
        }
    }
}

/// Represents a parsed font
#[derive(Debug)]
pub struct ParsedFont {
    names: Names,
    weight: FontWeight,
    width: FontWidth,
    italic: bool,
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
        Self::from_face(&face)
    }

    pub fn from_face(face: &crate::ftwrap::Face) -> anyhow::Result<Self> {
        let italic = face.italic();
        let (weight, width) = face.weight_and_width();
        let weight = FontWeight::from_opentype_weight(weight);
        let width = FontWidth::from_opentype_width(width);

        Ok(Self {
            names: Names::from_ft_face(&face),
            weight,
            width,
            italic,
        })
    }

    pub fn names(&self) -> &Names {
        &self.names
    }

    pub fn weight(&self) -> FontWeight {
        self.weight
    }

    pub fn width(&self) -> FontWidth {
        self.width
    }

    pub fn italic(&self) -> bool {
        self.italic
    }

    pub fn matches_attributes(&self, attr: &FontAttributes) -> FontMatch {
        if let Some(fam) = self.names.family.as_ref() {
            if attr.family == *fam {
                let wanted_width = FontWidth::Normal;
                if wanted_width == self.width {
                    let wanted_weight = if attr.bold {
                        FontWeight::Bold
                    } else {
                        FontWeight::Regular
                    }
                    .to_opentype_weight();
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Ord, PartialOrd)]
pub enum FontMatch {
    Weight(u16),
    FullName,
    NoMatch,
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
    let mut locator = FontDataHandle::Memory {
        name: "".to_string(),
        data: data.clone(),
        index: 0,
        variation: 0,
    };

    let num_faces = lib.query_num_faces(&locator)?;

    for index in 0..num_faces {
        locator.set_index(index);
        let face = lib.face_from_locator(&locator)?;
        let parsed = ParsedFont::from_face(&face)?;

        if parsed.matches_attributes(attr) != FontMatch::NoMatch {
            return Ok(Some(index as usize));
        }
    }
    Ok(None)
}

/// In case the user has a broken configuration, or no configuration,
/// we bundle JetBrains Mono and Noto Color Emoji to act as reasonably
/// sane fallback fonts.
/// This function loads those.
pub(crate) fn load_built_in_fonts(
    font_info: &mut Vec<(ParsedFont, FontDataHandle)>,
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
        let parsed = ParsedFont::from_face(&face)?;
        font_info.push((
            parsed,
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
    font_info: &mut Vec<(ParsedFont, FontDataHandle)>,
) -> anyhow::Result<()> {
    let lib = crate::ftwrap::Library::new()?;

    let locator = FontDataHandle::OnDisk {
        path: path.to_path_buf(),
        index: 0,
        variation: 0,
    };

    let num_faces = lib.query_num_faces(&locator)?;

    fn load_one(
        lib: &crate::ftwrap::Library,
        path: &Path,
        index: u32,
        font_info: &mut Vec<(ParsedFont, FontDataHandle)>,
    ) -> anyhow::Result<()> {
        let locator = FontDataHandle::OnDisk {
            path: path.to_path_buf(),
            index,
            variation: 0,
        };

        let face = lib.face_from_locator(&locator)?;
        if let Ok(variations) = face.variations() {
            for (variation, parsed) in variations.into_iter().enumerate() {
                font_info.push((
                    parsed,
                    FontDataHandle::OnDisk {
                        path: path.to_path_buf(),
                        index,
                        variation: variation as u32 + 1,
                    },
                ));
            }
        } else {
            let parsed = ParsedFont::from_locator(&locator)?;
            font_info.push((parsed, locator));
        }
        Ok(())
    }

    for index in 0..num_faces {
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
