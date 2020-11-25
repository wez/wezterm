use crate::locator::FontDataHandle;
use crate::parser::*;
use crate::shaper::{FallbackIdx, FontMetrics, FontShaper, GlyphInfo};
use anyhow::{anyhow, bail};

pub struct AllsortsShaper {
    fonts: Vec<Option<ParsedFont>>,
}

impl AllsortsShaper {
    pub fn new(handles: &[FontDataHandle]) -> anyhow::Result<Self> {
        let mut fonts = vec![];
        let mut success = false;
        for handle in handles {
            match ParsedFont::from_locator(handle) {
                Ok(font) => {
                    fonts.push(Some(font));
                    success = true;
                }
                Err(err) => {
                    log::warn!("Failed to parse {:?}: {}", handle, err);
                    fonts.push(None);
                }
            }
        }
        if !success {
            bail!("failed to load any fonts in this fallback set!?");
        }
        Ok(Self { fonts })
    }

    #[allow(clippy::too_many_arguments)]
    fn shape_into(
        &self,
        font_index: FallbackIdx,
        s: &str,
        slice_index: usize,
        script: u32,
        lang: u32,
        font_size: f64,
        dpi: u32,
        results: &mut Vec<GlyphInfo>,
        no_glyphs: &mut Vec<char>,
    ) -> anyhow::Result<()> {
        let font = match self.fonts.get(font_index) {
            Some(Some(font)) => font,
            Some(None) => {
                return self.shape_into(
                    font_index + 1,
                    s,
                    slice_index,
                    script,
                    lang,
                    font_size,
                    dpi,
                    results,
                    no_glyphs,
                );
            }
            None => {
                // Note: since we added a last resort font, this case shouldn't
                // ever get hit in practice.
                // We ran out of fallback fonts, so use a replacement
                // character that is likely to be in one of those fonts.
                let mut alt_text = String::new();
                for c in s.chars() {
                    no_glyphs.push(c);
                    alt_text.push('?');
                }
                if alt_text == s {
                    // We already tried to fallback to this and failed
                    return Err(anyhow!("could not fallback to ? character"));
                }
                return self.shape_into(
                    0,
                    &alt_text,
                    slice_index,
                    script,
                    lang,
                    font_size,
                    dpi,
                    results,
                    no_glyphs,
                );
            }
        };

        if font_index + 1 == self.fonts.len() {
            // We are the last resort font, so each codepoint is considered
            // to be worthy of a fallback lookup
            for c in s.chars() {
                no_glyphs.push(c);
            }
        }

        let first_pass =
            font.shape_text(s, slice_index, font_index, script, lang, font_size, dpi)?;

        let mut item_iter = first_pass.into_iter();
        while let Some(item) = item_iter.next() {
            match item {
                MaybeShaped::Resolved(info) => {
                    results.push(info);
                }
                MaybeShaped::Unresolved { raw, slice_start } => {
                    // There was no glyph in that font, so we'll need to shape
                    // using a fallback.  Let's collect together any potential
                    // run of unresolved entries first
                    self.shape_into(
                        font_index + 1,
                        &raw,
                        slice_start,
                        script,
                        lang,
                        font_size,
                        dpi,
                        results,
                        no_glyphs,
                    )?;
                }
            }
        }

        Ok(())
    }
}

impl FontShaper for AllsortsShaper {
    fn shape(
        &self,
        text: &str,
        size: f64,
        dpi: u32,
        no_glyphs: &mut Vec<char>,
    ) -> anyhow::Result<Vec<GlyphInfo>> {
        let mut results = vec![];
        let script = allsorts::tag::LATN;
        let lang = allsorts::tag::DFLT;
        self.shape_into(0, text, 0, script, lang, size, dpi, &mut results, no_glyphs)?;
        // log::error!("shape {} into {:?}", text, results);
        Ok(results)
    }

    fn metrics_for_idx(&self, font_idx: usize, size: f64, dpi: u32) -> anyhow::Result<FontMetrics> {
        let font = self
            .fonts
            .get(font_idx)
            .ok_or_else(|| anyhow::anyhow!("invalid font_idx {}", font_idx))?;
        let font = font
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("failed to load font_idx {}", font_idx))?;
        Ok(font.get_metrics(size, dpi))
    }

    fn metrics(&self, size: f64, dpi: u32) -> anyhow::Result<FontMetrics> {
        for font in &self.fonts {
            if let Some(font) = font {
                return Ok(font.get_metrics(size, dpi));
            }
        }
        bail!("no fonts available for collecting metrics!?");
    }
}
