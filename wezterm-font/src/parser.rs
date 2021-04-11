use crate::locator::{FontDataHandle, FontDataSource};
use crate::shaper::GlyphInfo;
use config::FontAttributes;
pub use config::{FontStretch, FontWeight};
use rangeset::RangeSet;
use std::cmp::Ordering;
use std::sync::Mutex;

#[derive(Debug)]
pub enum MaybeShaped {
    Resolved(GlyphInfo),
    Unresolved { raw: String, slice_start: usize },
}

/// Represents a parsed font
#[derive(Debug)]
pub struct ParsedFont {
    names: Names,
    weight: FontWeight,
    stretch: FontStretch,
    italic: bool,
    pub handle: FontDataHandle,
    coverage: Mutex<RangeSet<u32>>,
}

impl Clone for ParsedFont {
    fn clone(&self) -> Self {
        Self {
            names: self.names.clone(),
            weight: self.weight,
            stretch: self.stretch,
            italic: self.italic,
            handle: self.handle.clone(),
            coverage: Mutex::new(self.coverage.lock().unwrap().clone()),
        }
    }
}

impl Eq for ParsedFont {}

impl PartialEq for ParsedFont {
    fn eq(&self, rhs: &Self) -> bool {
        self.stretch == rhs.stretch
            && self.weight == rhs.weight
            && self.italic == rhs.italic
            && self.names == rhs.names
    }
}

impl Ord for ParsedFont {
    fn cmp(&self, rhs: &Self) -> Ordering {
        match self.stretch.cmp(&rhs.stretch) {
            o @ Ordering::Less | o @ Ordering::Greater => o,
            Ordering::Equal => match self.weight.cmp(&rhs.weight) {
                o @ Ordering::Less | o @ Ordering::Greater => o,
                Ordering::Equal => match self.italic.cmp(&rhs.italic) {
                    o @ Ordering::Less | o @ Ordering::Greater => o,
                    Ordering::Equal => self.names.cmp(&rhs.names),
                },
            },
        }
    }
}

impl PartialOrd for ParsedFont {
    fn partial_cmp(&self, rhs: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(rhs))
    }
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
            coverage: Mutex::new(RangeSet::new()),
        })
    }

    /// Computes the intersection of the wanted set of codepoints with
    /// the set of codepoints covered by this font entry.
    /// Computes the codepoint coverage for this font entry if we haven't
    /// already done so.
    pub fn coverage_intersection(&self, wanted: &RangeSet<u32>) -> anyhow::Result<RangeSet<u32>> {
        let mut cov = self.coverage.lock().unwrap();
        if cov.is_empty() {
            let t = std::time::Instant::now();
            let lib = crate::ftwrap::Library::new()?;
            let face = lib.face_from_locator(&self.handle)?;
            *cov = face.compute_coverage();
            let elapsed = t.elapsed();
            metrics::histogram!("font.compute.codepoint.coverage", elapsed);
            log::debug!(
                "{} codepoint coverage computed in {:?}",
                self.names.full_name,
                elapsed
            );
        }
        Ok(wanted.intersection(&cov))
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

    pub fn matches_name(&self, attr: &FontAttributes) -> bool {
        if let Some(fam) = self.names.family.as_ref() {
            if attr.family == *fam {
                return true;
            }
        }
        self.matches_full_or_ps_name(attr)
    }

    pub fn matches_full_or_ps_name(&self, attr: &FontAttributes) -> bool {
        if attr.family == self.names.full_name {
            return true;
        }
        if let Some(ps) = self.names.postscript_name.as_ref() {
            if attr.family == *ps {
                return true;
            }
        }
        false
    }

    /// Perform CSS Fonts Level 3 font matching.
    /// This implementation is derived from the `find_best_match` function
    /// in the font-kit crate which is
    /// Copyright © 2018 The Pathfinder Project Developers.
    /// https://drafts.csswg.org/css-fonts-3/#font-style-matching says
    pub fn best_matching_index<P: std::ops::Deref<Target = Self> + std::fmt::Debug>(
        attr: &FontAttributes,
        fonts: &[P],
    ) -> Option<usize> {
        if fonts.is_empty() {
            return None;
        }

        let mut candidates: Vec<usize> = (0..fonts.len()).collect();

        // First, filter by stretch
        let stretch_value = attr.stretch.to_opentype_stretch();
        let stretch = if candidates
            .iter()
            .any(|&idx| fonts[idx].stretch == attr.stretch)
        {
            attr.stretch
        } else if attr.stretch <= FontStretch::Normal {
            // Find the closest stretch, looking at narrower first before
            // looking at wider candidates
            match candidates
                .iter()
                .filter(|&&idx| fonts[idx].stretch < attr.stretch)
                .min_by_key(|&&idx| stretch_value - fonts[idx].stretch.to_opentype_stretch())
            {
                Some(&idx) => fonts[idx].stretch,
                None => {
                    let idx = *candidates.iter().min_by_key(|&&idx| {
                        fonts[idx].stretch.to_opentype_stretch() - stretch_value
                    })?;
                    fonts[idx].stretch
                }
            }
        } else {
            // Look at wider values, then narrower values
            match candidates
                .iter()
                .filter(|&&idx| fonts[idx].stretch > attr.stretch)
                .min_by_key(|&&idx| fonts[idx].stretch.to_opentype_stretch() - stretch_value)
            {
                Some(&idx) => fonts[idx].stretch,
                None => {
                    let idx = *candidates.iter().min_by_key(|&&idx| {
                        stretch_value - fonts[idx].stretch.to_opentype_stretch()
                    })?;
                    fonts[idx].stretch
                }
            }
        };

        // Reduce to matching stretches
        candidates.retain(|&idx| fonts[idx].stretch == stretch);

        // Now match style: italics
        let styles = [attr.italic, !attr.italic];
        let italic = *styles
            .iter()
            .filter(|&&italic| candidates.iter().any(|&idx| fonts[idx].italic == italic))
            .next()?;

        // Reduce to matching italics
        candidates.retain(|&idx| fonts[idx].italic == italic);

        // And now match by font weight
        let query_weight = attr.weight.to_opentype_weight();
        let weight = if candidates
            .iter()
            .any(|&idx| fonts[idx].weight == attr.weight)
        {
            // Exact match for the requested weight
            attr.weight
        } else if attr.weight == FontWeight::Regular
            && candidates
                .iter()
                .any(|&idx| fonts[idx].weight == FontWeight::Medium)
        {
            // https://drafts.csswg.org/css-fonts-3/#font-style-matching says
            // that if they want weight=400 and we don't have 400,
            // look at weight 500 first
            FontWeight::Medium
        } else if attr.weight == FontWeight::Medium
            && candidates
                .iter()
                .any(|&idx| fonts[idx].weight == FontWeight::Regular)
        {
            // Similarly, look at regular before Medium if they wanted
            // Medium and we didn't have it
            FontWeight::Regular
        } else if attr.weight <= FontWeight::Medium {
            // Find best lighter weight, else best heavier weight
            match candidates
                .iter()
                .filter(|&&idx| fonts[idx].weight <= attr.weight)
                .min_by_key(|&&idx| query_weight - fonts[idx].weight.to_opentype_weight())
            {
                Some(&idx) => fonts[idx].weight,
                None => {
                    let idx = *candidates.iter().min_by_key(|&&idx| {
                        fonts[idx].weight.to_opentype_weight() - query_weight
                    })?;
                    fonts[idx].weight
                }
            }
        } else {
            // Find best heavier weight, else best lighter weight
            match candidates
                .iter()
                .filter(|&&idx| fonts[idx].weight >= attr.weight)
                .min_by_key(|&&idx| fonts[idx].weight.to_opentype_weight() - query_weight)
            {
                Some(&idx) => fonts[idx].weight,
                None => {
                    let idx = *candidates.iter().min_by_key(|&&idx| {
                        query_weight - fonts[idx].weight.to_opentype_weight()
                    })?;
                    fonts[idx].weight
                }
            }
        };

        // Reduce to matching weight
        candidates.retain(|&idx| fonts[idx].weight == weight);

        // The first one in this set is our best match
        candidates.into_iter().next()
    }

    pub fn best_match(attr: &FontAttributes, mut fonts: Vec<Self>) -> Option<Self> {
        let refs: Vec<&Self> = fonts.iter().collect();
        let idx = Self::best_matching_index(attr, &refs)?;
        fonts.drain(idx..=idx).next()
    }
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
            source: FontDataSource::BuiltIn { data, name },
            index: 0,
            variation: 0,
        };
        let face = lib.face_from_locator(&locator)?;
        let parsed = ParsedFont::from_face(&face, locator)?;
        font_info.push(parsed);
    }

    Ok(())
}

pub fn best_matching_font(
    source: &FontDataSource,
    font_attr: &FontAttributes,
) -> anyhow::Result<Option<ParsedFont>> {
    let mut font_info = vec![];
    parse_and_collect_font_info(source, &mut font_info)?;
    Ok(ParsedFont::best_match(font_attr, font_info))
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
