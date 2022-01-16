use crate::locator::{FontDataHandle, FontDataSource, FontOrigin};
use crate::shaper::GlyphInfo;
use config::{FontAttributes, FreeTypeLoadFlags, FreeTypeLoadTarget};
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
pub struct ParsedFont {
    names: Names,
    weight: FontWeight,
    stretch: FontStretch,
    italic: bool,
    cap_height: Option<f64>,
    pub handle: FontDataHandle,
    coverage: Mutex<RangeSet<u32>>,
    pub synthesize_italic: bool,
    pub synthesize_bold: bool,
    pub synthesize_dim: bool,
    pub assume_emoji_presentation: bool,
    pub pixel_sizes: Vec<u16>,

    pub harfbuzz_features: Option<Vec<String>>,
    pub freetype_load_target: Option<FreeTypeLoadTarget>,
    pub freetype_render_target: Option<FreeTypeLoadTarget>,
    pub freetype_load_flags: Option<FreeTypeLoadFlags>,
}

impl std::fmt::Debug for ParsedFont {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        fmt.debug_struct("ParsedFont")
            .field("names", &self.names)
            .field("weight", &self.weight)
            .field("stretch", &self.stretch)
            .field("italic", &self.italic)
            .field("handle", &self.handle)
            .field("cap_height", &self.cap_height)
            .field("synthesize_italic", &self.synthesize_italic)
            .field("synthesize_bold", &self.synthesize_bold)
            .field("synthesize_dim", &self.synthesize_dim)
            .field("assume_emoji_presentation", &self.assume_emoji_presentation)
            .field("pixel_sizes", &self.pixel_sizes)
            .finish()
    }
}

impl Clone for ParsedFont {
    fn clone(&self) -> Self {
        Self {
            names: self.names.clone(),
            weight: self.weight,
            stretch: self.stretch,
            italic: self.italic,
            synthesize_italic: self.synthesize_italic,
            synthesize_bold: self.synthesize_bold,
            synthesize_dim: self.synthesize_dim,
            assume_emoji_presentation: self.assume_emoji_presentation,
            handle: self.handle.clone(),
            cap_height: self.cap_height.clone(),
            coverage: Mutex::new(self.coverage.lock().unwrap().clone()),
            pixel_sizes: self.pixel_sizes.clone(),
            harfbuzz_features: self.harfbuzz_features.clone(),
            freetype_load_target: self.freetype_load_target,
            freetype_render_target: self.freetype_render_target,
            freetype_load_flags: self.freetype_load_flags,
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
        match self.names.family.cmp(&rhs.names.family) {
            o @ Ordering::Less | o @ Ordering::Greater => o,
            Ordering::Equal => match self.stretch.cmp(&rhs.stretch) {
                o @ Ordering::Less | o @ Ordering::Greater => o,
                Ordering::Equal => match self.weight.cmp(&rhs.weight) {
                    o @ Ordering::Less | o @ Ordering::Greater => o,
                    Ordering::Equal => match self.italic.cmp(&rhs.italic) {
                        o @ Ordering::Less | o @ Ordering::Greater => o,
                        Ordering::Equal => self.handle.cmp(&rhs.handle),
                    },
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
    pub family: String,
    pub sub_family: Option<String>,
    pub postscript_name: Option<String>,
    pub aliases: Vec<String>,
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

        let aliases = face.get_sfnt_names();

        Names {
            full_name,
            family,
            sub_family: Some(sub_family),
            postscript_name: Some(postscript_name),
            aliases,
        }
    }
}

impl ParsedFont {
    pub fn from_locator(handle: &FontDataHandle) -> anyhow::Result<Self> {
        let lib = crate::ftwrap::Library::new()?;
        let face = lib.face_from_locator(handle)?;
        Self::from_face(&face, handle.clone())
    }

    pub fn lua_name(&self) -> String {
        format!(
            "wezterm.font(\"{}\", {{weight={}, stretch=\"{}\", italic={}}})",
            self.names.family, self.weight, self.stretch, self.italic
        )
    }

    pub fn lua_fallback(handles: &[Self]) -> String {
        let mut code = "wezterm.font_with_fallback({\n".to_string();

        for p in handles {
            code.push_str(&format!("  -- {}\n", p.handle.diagnostic_string()));
            if p.synthesize_italic {
                code.push_str("  -- Will synthesize italics\n");
            }
            if p.synthesize_bold {
                code.push_str("  -- Will synthesize bold\n");
            } else if p.synthesize_dim {
                code.push_str("  -- Will synthesize dim\n");
            }
            if p.assume_emoji_presentation {
                code.push_str("  -- Assumed to have Emoji Presentation\n");
            }
            if !p.pixel_sizes.is_empty() {
                code.push_str(&format!("  -- Pixel sizes: {:?}\n", p.pixel_sizes));
            }

            if p.weight == FontWeight::REGULAR
                && p.stretch == FontStretch::Normal
                && !p.italic
                && p.freetype_render_target.is_none()
                && p.freetype_load_target.is_none()
                && p.freetype_load_flags.is_none()
                && p.harfbuzz_features.is_none()
            {
                code.push_str(&format!("  \"{}\",\n", p.names.family));
            } else {
                code.push_str(&format!("  {{family=\"{}\"", p.names.family));
                if p.weight != FontWeight::REGULAR {
                    code.push_str(&format!(", weight={}", p.weight));
                }
                if p.stretch != FontStretch::Normal {
                    code.push_str(&format!(", stretch=\"{}\"", p.stretch));
                }
                if p.italic {
                    code.push_str(", italic=true");
                }
                if let Some(item) = p.freetype_load_flags {
                    code.push_str(&format!(", freetype_load_flags=\"{}\"", item.to_string()));
                }
                if let Some(item) = p.freetype_load_target {
                    code.push_str(&format!(", freetype_load_target=\"{:?}\"", item));
                }
                if let Some(item) = p.freetype_render_target {
                    code.push_str(&format!(", freetype_render_target=\"{:?}\"", item));
                }
                if let Some(feat) = &p.harfbuzz_features {
                    code.push_str(", harfbuzz_features={");
                    for (idx, f) in feat.iter().enumerate() {
                        if idx > 0 {
                            code.push_str(", ");
                        }
                        code.push('"');
                        code.push_str(f);
                        code.push('"');
                    }
                    code.push('}');
                }
                code.push_str("},\n")
            }
            code.push_str("\n");
        }
        code.push_str("})");
        code
    }

    pub fn from_face(face: &crate::ftwrap::Face, handle: FontDataHandle) -> anyhow::Result<Self> {
        let italic = face.italic();
        let (ot_weight, width) = face.weight_and_width();
        let weight = FontWeight::from_opentype_weight(ot_weight);
        let stretch = FontStretch::from_opentype_stretch(width);
        let cap_height = face.cap_height();
        let pixel_sizes = face.pixel_sizes();
        let has_color = unsafe {
            (((*face.face).face_flags as u32) & (crate::ftwrap::FT_FACE_FLAG_COLOR as u32)) != 0
        };
        let assume_emoji_presentation = has_color;

        let names = Names::from_ft_face(&face);
        let italic = italic || {
            // Objectively gross, but freetype's italic property isn't always
            // set for italic fonts.
            // fontconfig resorts to name matching, so we do too :-/
            let lower = names.full_name.to_lowercase();
            lower.contains("italic") || lower.contains("kursiv")
        };

        Ok(Self {
            names,
            weight,
            stretch,
            italic,
            synthesize_italic: false,
            synthesize_bold: false,
            synthesize_dim: false,
            assume_emoji_presentation,
            handle,
            coverage: Mutex::new(RangeSet::new()),
            cap_height,
            pixel_sizes,
            harfbuzz_features: None,
            freetype_render_target: None,
            freetype_load_target: None,
            freetype_load_flags: None,
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
        if attr.family == self.names.family {
            return true;
        }
        self.matches_full_or_ps_name(attr) || self.matches_alias(attr)
    }

    pub fn matches_alias(&self, attr: &FontAttributes) -> bool {
        for a in &self.names.aliases {
            if *a == attr.family {
                return true;
            }
        }
        false
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
        pixel_size: u16,
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
        } else if attr.weight == FontWeight::REGULAR
            && candidates
                .iter()
                .any(|&idx| fonts[idx].weight == FontWeight::MEDIUM)
        {
            // https://drafts.csswg.org/css-fonts-3/#font-style-matching says
            // that if they want weight=400 and we don't have 400,
            // look at weight 500 first
            FontWeight::MEDIUM
        } else if attr.weight == FontWeight::MEDIUM
            && candidates
                .iter()
                .any(|&idx| fonts[idx].weight == FontWeight::REGULAR)
        {
            // Similarly, look at regular before Medium if they wanted
            // Medium and we didn't have it
            FontWeight::REGULAR
        } else if attr.weight <= FontWeight::MEDIUM {
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

        // Check for best matching pixel strike
        if let Some((_distance, idx)) = candidates
            .iter()
            .map(|&idx| {
                let distance = fonts[idx]
                    .pixel_sizes
                    .iter()
                    .map(|&size| ((pixel_size as i32) - (size as i32)).abs())
                    .min()
                    .unwrap_or(i32::MAX);
                (distance, idx)
            })
            .min()
        {
            return Some(idx);
        }

        // The first one in this set is our best match
        candidates.into_iter().next()
    }

    pub fn best_match(
        attr: &FontAttributes,
        pixel_size: u16,
        mut fonts: Vec<Self>,
    ) -> Option<Self> {
        let refs: Vec<&Self> = fonts.iter().collect();
        let idx = Self::best_matching_index(attr, &refs, pixel_size)?;
        fonts.drain(idx..=idx).next().map(|p| p.synthesize(attr))
    }

    /// Update self to reflect whether the rasterizer might need to synthesize
    /// italic for this font.
    pub fn synthesize(mut self, attr: &FontAttributes) -> Self {
        self.harfbuzz_features = attr.harfbuzz_features.clone();
        self.freetype_render_target = attr.freetype_render_target;
        self.freetype_load_target = attr.freetype_load_target;
        self.freetype_load_flags = attr.freetype_load_flags;

        self.synthesize_italic = !self.italic && attr.italic;
        self.synthesize_bold = attr.weight >= FontWeight::BOLD
            && attr.weight > self.weight
            && self.weight <= FontWeight::REGULAR;
        self.synthesize_dim = attr.weight < FontWeight::REGULAR
            && attr.weight < self.weight
            && self.weight >= FontWeight::REGULAR;
        self
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
        font!("../../assets/fonts/Roboto-Black.ttf"),
        font!("../../assets/fonts/Roboto-BlackItalic.ttf"),
        font!("../../assets/fonts/Roboto-Bold.ttf"),
        font!("../../assets/fonts/Roboto-BoldItalic.ttf"),
        font!("../../assets/fonts/Roboto-Italic.ttf"),
        font!("../../assets/fonts/Roboto-Light.ttf"),
        font!("../../assets/fonts/Roboto-LightItalic.ttf"),
        font!("../../assets/fonts/Roboto-Medium.ttf"),
        font!("../../assets/fonts/Roboto-MediumItalic.ttf"),
        font!("../../assets/fonts/Roboto-Regular.ttf"),
        font!("../../assets/fonts/Roboto-Thin.ttf"),
        font!("../../assets/fonts/Roboto-ThinItalic.ttf"),
        font!("../../assets/fonts/NotoColorEmoji.ttf"),
        font!("../../assets/fonts/Symbols-Nerd-Font-Mono.ttf"),
        font!("../../assets/fonts/LastResortHE-Regular.ttf"),
    ] {
        let locator = FontDataHandle {
            source: FontDataSource::BuiltIn { data, name },
            index: 0,
            variation: 0,
            origin: FontOrigin::BuiltIn,
            coverage: None,
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
    origin: FontOrigin,
    pixel_size: u16,
) -> anyhow::Result<Option<ParsedFont>> {
    let mut font_info = vec![];
    parse_and_collect_font_info(source, &mut font_info, origin)?;
    font_info.retain(|font| font.matches_name(font_attr));
    Ok(ParsedFont::best_match(font_attr, pixel_size, font_info))
}

pub(crate) fn parse_and_collect_font_info(
    source: &FontDataSource,
    font_info: &mut Vec<ParsedFont>,
    origin: FontOrigin,
) -> anyhow::Result<()> {
    let lib = crate::ftwrap::Library::new()?;
    let num_faces = lib.query_num_faces(&source)?;

    fn load_one(
        lib: &crate::ftwrap::Library,
        source: &FontDataSource,
        index: u32,
        font_info: &mut Vec<ParsedFont>,
        origin: &FontOrigin,
    ) -> anyhow::Result<()> {
        let locator = FontDataHandle {
            source: source.clone(),
            index,
            variation: 0,
            origin: origin.clone(),
            coverage: None,
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
        if let Err(err) = load_one(&lib, &source, index, font_info, &origin) {
            log::trace!("error while parsing {:?} index {}: {}", source, index, err);
        }
    }

    Ok(())
}
