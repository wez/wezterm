//! This module analyzes an image to determine a set of
//! unique-within-threshold colors.
//! The technique used is to convert the colors to the
//! LAB colorspace, which is a perceptually uniform colorspace,
//! and then apply DeltaE to measure the difference between
//! color candidates.
//! This is computationally expensive so a couple of techniques
//! are used to minimize the search space:
//! * fuzziness is used to avoid looking at every pixel
//! * The image is resized smaller to reduce the total number
//!   of pixel candidates
//! The results are cached to avoid recomputing on each
//! evaluation of the config file.
use crate::ColorWrap;
use config::lua::mlua::{self, Lua};
use config::SrgbaTuple;
use deltae::LabValue;
use image::Pixel;
use lru::LruCache;
use luahelper::impl_lua_conversion_dynamic;
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::Mutex;
use std::time::SystemTime;
use wezterm_dynamic::{FromDynamic, ToDynamic};

#[derive(FromDynamic, ToDynamic, Debug, Clone, Copy)]
pub struct ExtractColorParams {
    #[dynamic(default = "default_threshold")]
    threshold: f32,
    #[dynamic(default = "default_min_brightness")]
    min_brightness: f32,
    #[dynamic(default = "default_max_brightness")]
    max_brightness: f32,
    #[dynamic(default = "default_num_colors")]
    num_colors: usize,
    #[dynamic(default = "default_fuzziness")]
    fuzziness: usize,
    #[dynamic(default = "default_max_width")]
    max_width: u16,
    #[dynamic(default = "default_max_height")]
    max_height: u16,
    #[dynamic(default = "default_min_contrast")]
    min_contrast: f32,
}
impl_lua_conversion_dynamic!(ExtractColorParams);

impl PartialEq for ExtractColorParams {
    fn eq(&self, rhs: &Self) -> bool {
        self.threshold == rhs.threshold
            && self.min_brightness == rhs.min_brightness
            && self.max_brightness == rhs.max_brightness
            && self.num_colors == rhs.num_colors
            && self.fuzziness == rhs.fuzziness
            && self.max_width == rhs.max_width
            && self.max_height == rhs.max_height
            && self.min_contrast == rhs.min_contrast
    }
}

impl Eq for ExtractColorParams {}

impl std::hash::Hash for ExtractColorParams {
    fn hash<H>(&self, hasher: &mut H)
    where
        H: std::hash::Hasher,
    {
        self.threshold.to_ne_bytes().hash(hasher);
        self.min_brightness.to_ne_bytes().hash(hasher);
        self.max_brightness.to_ne_bytes().hash(hasher);
        self.num_colors.hash(hasher);
        self.fuzziness.hash(hasher);
        self.max_width.hash(hasher);
        self.max_height.hash(hasher);
        self.min_contrast.to_ne_bytes().hash(hasher);
    }
}

fn default_threshold() -> f32 {
    50.
}

fn default_min_brightness() -> f32 {
    0.
}

fn default_min_contrast() -> f32 {
    0.0
}

fn default_max_brightness() -> f32 {
    90.0
}

fn default_num_colors() -> usize {
    16
}

fn default_fuzziness() -> usize {
    5
}

fn default_max_width() -> u16 {
    640
}

fn default_max_height() -> u16 {
    480
}

impl Default for ExtractColorParams {
    fn default() -> Self {
        Self {
            threshold: default_threshold(),
            min_brightness: default_min_brightness(),
            max_brightness: default_max_brightness(),
            num_colors: default_num_colors(),
            fuzziness: default_fuzziness(),
            max_width: default_max_width(),
            max_height: default_max_height(),
            min_contrast: default_min_contrast(),
        }
    }
}

struct CachedAnalysis {
    modified: SystemTime,
    colors: Vec<ColorWrap>,
}

lazy_static::lazy_static! {
    static ref IMG_COLOR_CACHE: Mutex<LruCache<(String, ExtractColorParams), CachedAnalysis>> = Mutex::new(LruCache::new(NonZeroUsize::new(16).unwrap()));
}

fn color_diff_lab(a: &LabValue, b: &LabValue) -> f32 {
    *deltae::DeltaE::new(a, b, deltae::DEMethod::DE2000).value()
}

fn contrast_ratio(a: &LabValue, b: &LabValue) -> f32 {
    let a = a.l + 5.;
    let b = b.l + 5.;
    if a > b {
        a / b
    } else {
        b / a
    }
}

fn extract_distinct_colors_lab(
    pool: &[LabValue],
    threshold: f32,
    min_brightness: f32,
    max_brightness: f32,
    min_contrast: f32,
    result: &mut Vec<LabValue>,
    limit: usize,
) {
    for candidate in pool {
        if limit > 0 && result.len() >= limit {
            return;
        }

        if candidate.l < min_brightness || candidate.l > max_brightness {
            continue;
        }

        let exists = result
            .iter()
            .any(|exist| color_diff_lab(candidate, exist) < threshold);
        let good_contrast = min_contrast == 0.0
            || result
                .iter()
                .all(|exist| contrast_ratio(candidate, exist) >= min_contrast);

        if !exists && good_contrast {
            result.push(*candidate);
        }
    }
}

pub fn extract_colors_from_image<'lua>(
    _: &'lua Lua,
    (file_name, params): (String, Option<ExtractColorParams>),
) -> mlua::Result<Vec<ColorWrap>> {
    let params = params.unwrap_or_default();

    let modified = std::fs::metadata(&file_name)
        .and_then(|m| m.modified())
        .map_err(|err| {
            mlua::Error::external(format!(
                "error getting modified time for {file_name}: {err:#}"
            ))
        })?;

    let mut cache = IMG_COLOR_CACHE.lock().unwrap();
    if let Some(hit) = cache.get(&(file_name.clone(), params)) {
        if hit.modified == modified {
            return Ok(hit.colors.clone());
        }
    }

    log::trace!("loading image {file_name}");
    let im = image::ImageReader::open(&file_name)
        .map_err(|err| mlua::Error::external(format!("{err:#} while loading {file_name}")))?
        .decode()
        .map_err(|err| {
            mlua::Error::external(format!("{err:#} while decoding image from {file_name}"))
        })?
        .resize(
            params.max_width.into(),
            params.max_height.into(),
            image::imageops::FilterType::Triangle,
        )
        .into_rgba8();
    log::trace!("analyzing image {file_name}");

    let mut threshold = params.threshold;

    // Score the pixels by their frequency
    let mut color_freq = HashMap::new();
    for (_, _, pixel) in im.enumerate_pixels().step_by(params.fuzziness) {
        let count = color_freq.entry(pixel).or_insert(0);
        *count += 1;
    }

    // Sort by descending frequency.
    // Resolve ties by sorting by the pixel value;
    // that avoids non-determinism when the config is reloaded.
    let mut ordered_pixels: Vec<_> = color_freq.into_iter().collect();
    ordered_pixels.sort_by(|(ap, a), (bp, b)| (b, &bp.0).cmp(&(a, &ap.0)));

    // Produce the Lab equivalent color for the next stage of analysis
    let mut all_colors = vec![];
    for (pixel, _) in ordered_pixels {
        let channels = pixel.channels();
        let color = csscolorparser::Color::from_rgba8(channels[0], channels[1], channels[2], 255);
        let (l, a, b, _alpha) = color.to_lab();
        all_colors.push(LabValue {
            l: l as f32,
            a: a as f32,
            b: b as f32,
        });
    }

    log::trace!("collected {} colors", all_colors.len());

    let mut result = vec![];
    extract_distinct_colors_lab(
        &all_colors,
        threshold,
        params.min_brightness,
        params.max_brightness,
        params.min_contrast,
        &mut result,
        params.num_colors,
    );
    log::trace!("found {} distinct colors within threshold", result.len());

    // Delta-E values <= 1.0 are not perceptibly different to
    // the human eye, so we should just terminate when we get that low.
    if params.num_colors > 0 {
        while result.len() < params.num_colors && threshold > 2. {
            threshold -= 1.0;
            log::trace!(
                "extract_colors_from_image: reducing threshold to \
             {threshold} to satisfy request for {} colors",
                params.num_colors
            );

            extract_distinct_colors_lab(
                &all_colors,
                threshold,
                params.min_brightness,
                params.max_brightness,
                params.min_contrast,
                &mut result,
                params.num_colors,
            );
        }
    }

    log::trace!("converting colors to correct type");
    let colors: Vec<ColorWrap> = result
        .into_iter()
        .map(|color| {
            let color = csscolorparser::Color::from_lab(
                color.l.into(),
                color.a.into(),
                color.b.into(),
                1.0,
            );
            let tuple = SrgbaTuple(
                color.r as f32,
                color.g as f32,
                color.b as f32,
                color.a as f32,
            );
            ColorWrap(tuple.into())
        })
        .collect();
    log::trace!("colors are now correctly typed");

    if threshold < params.threshold {
        log::warn!(
            "extract_colors_from_image: adjusted threshold {} in order \
             to satisfy num_colors={}",
            threshold,
            params.num_colors
        );
    }

    if colors.len() < params.num_colors {
        return Err(mlua::Error::external(format!(
            "extract_colors_from_image: only found {} out of requested {} colors.",
            colors.len(),
            params.num_colors
        )));
    }

    cache.put(
        (file_name, params),
        CachedAnalysis {
            modified,
            colors: colors.clone(),
        },
    );

    Ok(colors)
}
