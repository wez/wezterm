use crate::locator::{FontDataHandle, FontDataSource, FontOrigin};
use crate::shaper::GlyphInfo;
use config::{FontAttributes, FontStyle, FreeTypeLoadFlags, FreeTypeLoadTarget};
pub use config::{FontStretch, FontWeight};
use rangeset::RangeSet;
use std::cmp::Ordering;
use std::sync::Mutex;

#[derive(Debug)]
pub enum MaybeShaped {
    Resolved(GlyphInfo),
    Unresolved { raw: String, slice_start: usize },
}

#[derive(Debug, Clone)]
pub struct FontPaletteInfo {
    pub name: String,
    pub palette_index: usize,
    pub usable_with_light_bg: bool,
    pub usable_with_dark_bg: bool,
}

/// Represents a parsed font
pub struct ParsedFont {
    names: Names,
    weight: FontWeight,
    stretch: FontStretch,
    style: FontStyle,
    cap_height: Option<f64>,
    pub handle: FontDataHandle,
    coverage: Mutex<RangeSet<u32>>,
    pub synthesize_italic: bool,
    pub synthesize_bold: bool,
    pub synthesize_dim: bool,
    pub assume_emoji_presentation: bool,
    pub pixel_sizes: Vec<u16>,
    pub is_built_in_fallback: bool,
    pub palettes: Vec<FontPaletteInfo>,

    pub harfbuzz_features: Option<Vec<String>>,
    pub freetype_load_target: Option<FreeTypeLoadTarget>,
    pub freetype_render_target: Option<FreeTypeLoadTarget>,
    pub freetype_load_flags: Option<FreeTypeLoadFlags>,
    pub scale: Option<f64>,
}

impl std::fmt::Debug for ParsedFont {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        fmt.debug_struct("ParsedFont")
            .field("names", &self.names)
            .field("weight", &self.weight)
            .field("stretch", &self.stretch)
            .field("style", &self.style)
            .field("handle", &self.handle)
            .field("cap_height", &self.cap_height)
            .field("synthesize_italic", &self.synthesize_italic)
            .field("synthesize_bold", &self.synthesize_bold)
            .field("synthesize_dim", &self.synthesize_dim)
            .field("assume_emoji_presentation", &self.assume_emoji_presentation)
            .field("pixel_sizes", &self.pixel_sizes)
            .field("harfbuzz_features", &self.harfbuzz_features)
            .field("freetype_load_target", &self.freetype_load_target)
            .field("freetype_render_target", &self.freetype_render_target)
            .field("freetype_load_flags", &self.freetype_load_flags)
            .field("scale", &self.scale)
            .finish()
    }
}

impl Clone for ParsedFont {
    fn clone(&self) -> Self {
        Self {
            names: self.names.clone(),
            weight: self.weight,
            stretch: self.stretch,
            style: self.style,
            synthesize_italic: self.synthesize_italic,
            synthesize_bold: self.synthesize_bold,
            synthesize_dim: self.synthesize_dim,
            assume_emoji_presentation: self.assume_emoji_presentation,
            handle: self.handle.clone(),
            cap_height: self.cap_height,
            coverage: Mutex::new(self.coverage.lock().unwrap().clone()),
            pixel_sizes: self.pixel_sizes.clone(),
            harfbuzz_features: self.harfbuzz_features.clone(),
            freetype_load_target: self.freetype_load_target,
            freetype_render_target: self.freetype_render_target,
            freetype_load_flags: self.freetype_load_flags,
            is_built_in_fallback: self.is_built_in_fallback,
            scale: self.scale,
            palettes: self.palettes.clone(),
        }
    }
}

impl Eq for ParsedFont {}

impl PartialEq for ParsedFont {
    fn eq(&self, rhs: &Self) -> bool {
        self.stretch == rhs.stretch
            && self.weight == rhs.weight
            && self.style == rhs.style
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
                    Ordering::Equal => match self.style.cmp(&rhs.style) {
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

/// Returns the "best" name from a set of records.
/// Best is English from a MS entry if available, as freetype's
/// source claims that a number of Mac entries have somewhat
/// broken encodings.
fn best_name(records: &[crate::ftwrap::NameRecord]) -> String {
    let mut win = None;
    let mut uni = None;
    let mut apple = None;

    for rec in records {
        match rec.platform_id as u32 {
            freetype::TT_PLATFORM_APPLE_UNICODE | freetype::TT_PLATFORM_ISO => {
                uni.replace(rec);
            }
            freetype::TT_PLATFORM_MACINTOSH => {
                apple.replace(rec);
            }
            freetype::TT_PLATFORM_MICROSOFT => {
                let is_english = (rec.language_id & 0x3ff) == 0x9;
                if is_english {
                    return rec.name.clone();
                }
                win.replace(rec);
            }
            _ => {}
        }
    }

    if let Some(rec) = apple {
        return rec.name.clone();
    }
    if let Some(rec) = win {
        return rec.name.clone();
    }
    if let Some(rec) = uni {
        return rec.name.clone();
    }
    records[0].name.clone()
}

/// Return a single name from a table.
/// The list of ids are tried in order: the first id with corresponding
/// names is taken, and the "best" of those names is returned.
fn name_from_table(
    names: &std::collections::HashMap<u32, Vec<crate::ftwrap::NameRecord>>,
    ids: &[u32],
) -> Option<String> {
    for id in ids {
        if let Some(name_list) = names.get(id) {
            return Some(best_name(name_list));
        }
    }
    None
}

/// Returns the sorted, deduplicated set of names across the list of ids
fn names_from_table(
    names: &std::collections::HashMap<u32, Vec<crate::ftwrap::NameRecord>>,
    ids: &[u32],
) -> Vec<String> {
    let mut result = vec![];

    for id in ids {
        if let Some(name_list) = names.get(id) {
            for rec in name_list {
                result.push(rec.name.clone());
            }
        }
    }
    result.sort();
    result.dedup();
    result
}

impl Names {
    pub fn from_ft_face(face: &crate::ftwrap::Face) -> Names {
        // We don't simply use the freetype functions to retrieve names,
        // as freetype has a limited set of encodings that it supports.
        // We process the name table for ourselves to increase our chances
        // of returning a good version of the name.
        // See <https://github.com/wezterm/wezterm/issues/1761#issuecomment-1079150560>
        // for a case where freetype returns `?????` for a name.
        let names = face.get_sfnt_names();

        let family = name_from_table(
            &names,
            &[
                freetype::TT_NAME_ID_TYPOGRAPHIC_FAMILY,
                freetype::TT_NAME_ID_FONT_FAMILY,
            ],
        )
        .unwrap_or_else(|| face.family_name());

        let sub_family = name_from_table(
            &names,
            &[
                freetype::TT_NAME_ID_TYPOGRAPHIC_SUBFAMILY,
                freetype::TT_NAME_ID_FONT_SUBFAMILY,
            ],
        )
        .unwrap_or_else(|| face.style_name());

        let postscript_name = name_from_table(&names, &[freetype::TT_NAME_ID_PS_NAME])
            .unwrap_or_else(|| face.postscript_name());

        let full_name = if sub_family.is_empty() {
            family.to_string()
        } else {
            format!("{} {}", family, sub_family)
        };

        let mut aliases = names_from_table(
            &names,
            &[
                freetype::TT_NAME_ID_TYPOGRAPHIC_FAMILY,
                freetype::TT_NAME_ID_FONT_FAMILY,
            ],
        );
        aliases.retain(|n| *n != full_name && *n != family);

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

    pub fn aka(&self) -> String {
        if self.names.aliases.is_empty() {
            String::new()
        } else {
            format!("(AKA: {}) ", self.names.aliases.join(", "))
        }
    }

    pub fn lua_name(&self) -> String {
        format!(
            "wezterm.font(\"{}\", {{weight={}, stretch=\"{}\", style=\"{}\"}})",
            self.names.family, self.weight, self.stretch, self.style
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
            if !p.palettes.is_empty() {
                for pal in &p.palettes {
                    let mut info = format!(
                        "  -- Palette: {} {}",
                        pal.palette_index,
                        pal.name.to_string()
                    );
                    if pal.usable_with_light_bg {
                        info.push_str(" (with light bg)");
                    }
                    if pal.usable_with_dark_bg {
                        info.push_str(" (with dark bg)");
                    }
                    info.push('\n');
                    code.push_str(&info);
                }
            }
            for aka in &p.names.aliases {
                code.push_str(&format!("  -- AKA: \"{}\"\n", aka));
            }

            if p.weight == FontWeight::REGULAR
                && p.stretch == FontStretch::Normal
                && p.style == FontStyle::Normal
                && p.freetype_render_target.is_none()
                && p.freetype_load_target.is_none()
                && p.freetype_load_flags.is_none()
                && p.harfbuzz_features.is_none()
                && p.scale.is_none()
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
                if p.style != FontStyle::Normal {
                    code.push_str(&format!(", style=\"{}\"", p.style));
                }
                if let Some(scale) = p.scale {
                    code.push_str(&format!(", scale={}", scale));
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
        let style = if face.italic() {
            FontStyle::Italic
        } else {
            FontStyle::Normal
        };
        let (ot_weight, width) = face.weight_and_width();
        let weight = FontWeight::from_opentype_weight(ot_weight);
        let stretch = FontStretch::from_opentype_stretch(width);
        let cap_height = face.cap_height();
        let pixel_sizes = face.pixel_sizes();

        let palettes = match face.get_palette_data() {
            Ok(info) => info
                .palettes
                .iter()
                .map(|p| FontPaletteInfo {
                    name: p.name.to_string(),
                    palette_index: p.palette_index,
                    usable_with_light_bg: (p.flags
                        & crate::ftwrap::FT_PALETTE_FOR_LIGHT_BACKGROUND as u16)
                        != 0,
                    usable_with_dark_bg: (p.flags
                        & crate::ftwrap::FT_PALETTE_FOR_DARK_BACKGROUND as u16)
                        != 0,
                })
                .collect(),
            Err(_) => vec![],
        };

        let has_svg = unsafe {
            (((*face.face).face_flags as u32) & (crate::ftwrap::FT_FACE_FLAG_SVG as u32)) != 0
        };

        if has_svg {
            if config::configuration().ignore_svg_fonts {
                anyhow::bail!("skipping svg font because ignore_svg_fonts=true");
            }
        }

        let has_color = unsafe {
            (((*face.face).face_flags as u32) & (crate::ftwrap::FT_FACE_FLAG_COLOR as u32)) != 0
        };
        let assume_emoji_presentation = has_color;

        let names = Names::from_ft_face(&face);
        // Objectively gross, but freetype's italic property is very coarse grained.
        // fontconfig resorts to name matching, so we do too :-/
        let style = match style {
            FontStyle::Normal => {
                let lower = names.full_name.to_lowercase();
                if lower.contains("italic") || lower.contains("kursiv") {
                    FontStyle::Italic
                } else if lower.contains("oblique") {
                    FontStyle::Oblique
                } else {
                    FontStyle::Normal
                }
            }
            FontStyle::Italic => {
                let lower = names.full_name.to_lowercase();
                if lower.contains("oblique") {
                    FontStyle::Oblique
                } else {
                    FontStyle::Italic
                }
            }
            // Currently "impossible" because freetype only knows italic or normal
            FontStyle::Oblique => FontStyle::Oblique,
        };

        let weight = match weight {
            FontWeight::REGULAR => {
                let lower = names.full_name.to_lowercase();
                let mut weight = weight;
                for (label, candidate) in &[
                    ("extrablack", FontWeight::EXTRABLACK),
                    // must match after other black variants
                    ("black", FontWeight::BLACK),
                    ("extrabold", FontWeight::EXTRABOLD),
                    ("demibold", FontWeight::DEMIBOLD),
                    // must match after other bold variants
                    ("bold", FontWeight::BOLD),
                    ("medium", FontWeight::MEDIUM),
                    ("book", FontWeight::BOOK),
                    ("demilight", FontWeight::DEMILIGHT),
                    ("extralight", FontWeight::EXTRALIGHT),
                    // must match after other light variants
                    ("light", FontWeight::LIGHT),
                    ("thin", FontWeight::THIN),
                ] {
                    if lower.contains(label) {
                        weight = *candidate;
                        break;
                    }
                }
                weight
            }
            weight => weight,
        };

        let stretch = match stretch {
            FontStretch::Normal => {
                let lower = names.full_name.to_lowercase();
                let mut stretch = stretch;
                for (label, value) in &[
                    ("ultracondensed", FontStretch::UltraCondensed),
                    ("extracondensed", FontStretch::ExtraCondensed),
                    ("semicondensed", FontStretch::SemiCondensed),
                    // must match after other condensed variants
                    ("condensed", FontStretch::Condensed),
                    ("semiexpanded", FontStretch::SemiExpanded),
                    ("extraexpanded", FontStretch::ExtraExpanded),
                    ("ultraexpanded", FontStretch::UltraExpanded),
                    // must match after other expanded variants
                    ("expanded", FontStretch::Expanded),
                ] {
                    if lower.contains(label) {
                        stretch = *value;
                        break;
                    }
                }

                stretch
            }
            stretch => stretch,
        };

        Ok(Self {
            names,
            weight,
            stretch,
            style,
            synthesize_italic: false,
            synthesize_bold: false,
            synthesize_dim: false,
            is_built_in_fallback: false,
            assume_emoji_presentation,
            handle,
            coverage: Mutex::new(RangeSet::new()),
            cap_height,
            pixel_sizes,
            harfbuzz_features: None,
            freetype_render_target: None,
            freetype_load_target: None,
            freetype_load_flags: None,
            scale: None,
            palettes,
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
            metrics::histogram!("font.compute.codepoint.coverage").record(elapsed);
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

    pub fn style(&self) -> FontStyle {
        self.style
    }

    pub fn matches_name(&self, attr: &FontAttributes) -> bool {
        if attr.family == self.names.family {
            return true;
        }
        if let Some(path) = self.handle.path_str() {
            if attr.family == path {
                return true;
            }
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

        // Now match style: italics.
        let styles = match attr.style {
            FontStyle::Normal => [FontStyle::Normal, FontStyle::Italic, FontStyle::Oblique],
            FontStyle::Italic => [FontStyle::Italic, FontStyle::Oblique, FontStyle::Normal],
            FontStyle::Oblique => [FontStyle::Oblique, FontStyle::Italic, FontStyle::Normal],
        };
        let style = *styles
            .iter()
            .find(|&&style| candidates.iter().any(|&idx| fonts[idx].style == style))?;

        // Reduce to matching italics
        candidates.retain(|&idx| fonts[idx].style == style);

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

        // Check for best matching pixel strike, but only if all
        // candidates have pixel strikes
        if candidates
            .iter()
            .all(|&idx| !fonts[idx].pixel_sizes.is_empty())
        {
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
        self.scale = attr.scale.map(|f| *f);

        self.synthesize_italic = self.style == FontStyle::Normal && attr.style != FontStyle::Normal;
        self.synthesize_bold = attr.weight >= FontWeight::DEMIBOLD
            && attr.weight > self.weight
            && self.weight <= FontWeight::REGULAR;
        self.synthesize_dim = attr.weight < FontWeight::REGULAR
            && attr.weight < self.weight
            && self.weight >= FontWeight::REGULAR;

        match attr.assume_emoji_presentation {
            Some(assume) => {
                self.assume_emoji_presentation = assume;
            }
            None => {
                // If they explicitly list an emoji font, assume that they
                // want it to be used for emoji presentation.
                // We match on "moji" rather than "emoji" as there are
                // emoji fonts that are moji rather than emoji :-/
                // This heuristic is awful, TBH.
                if !self.is_built_in_fallback
                    && !attr.is_synthetic
                    && self.names.full_name.to_lowercase().contains("moji")
                {
                    self.assume_emoji_presentation = true;
                }
            }
        }

        self
    }
}

/// In case the user has a broken configuration, or no configuration,
/// we bundle JetBrains Mono and Noto Color Emoji to act as reasonably
/// sane fallback fonts.
/// This function loads those.
pub(crate) fn load_built_in_fonts(font_info: &mut Vec<ParsedFont>) -> anyhow::Result<()> {
    #[allow(unused_macros)]
    macro_rules! font {
        ($font:literal) => {
            (include_bytes!($font) as &'static [u8], $font)
        };
    }
    let lib = crate::ftwrap::Library::new()?;

    let built_ins: &[&[(&[u8], &str)]] = &[
        #[cfg(any(test, feature = "vendor-jetbrains"))]
        &[
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
            font!("../../assets/fonts/JetBrainsMono-SemiBoldItalic.ttf"),
            font!("../../assets/fonts/JetBrainsMono-SemiBold.ttf"),
            font!("../../assets/fonts/JetBrainsMono-ThinItalic.ttf"),
            font!("../../assets/fonts/JetBrainsMono-Thin.ttf"),
        ],
        #[cfg(any(test, feature = "vendor-roboto"))]
        &[
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
        ],
        #[cfg(any(test, feature = "vendor-noto-emoji"))]
        &[font!("../../assets/fonts/NotoColorEmoji.ttf")],
        #[cfg(any(test, feature = "vendor-nerd-font-symbols"))]
        &[font!("../../assets/fonts/SymbolsNerdFontMono-Regular.ttf")],
    ];
    for bundle in built_ins {
        for (data, name) in bundle.iter() {
            let locator = FontDataHandle {
                source: FontDataSource::BuiltIn { data, name },
                index: 0,
                variation: 0,
                origin: FontOrigin::BuiltIn,
                coverage: None,
            };
            let face = lib.face_from_locator(&locator)?;
            let mut parsed = ParsedFont::from_face(&face, locator)?;
            parsed.is_built_in_fallback = true;
            font_info.push(parsed);
        }
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
