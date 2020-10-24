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
                );
            }
            None => {
                // We ran out of fallback fonts, so use a replacement
                // character that is likely to be in one of those fonts
                let mut alt_text = String::new();
                let num_chars = s.chars().count();
                for _ in 0..num_chars {
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
                );
            }
        };
        let first_pass =
            font.shape_text(s, slice_index, font_index, script, lang, font_size, dpi)?;

        let mut item_iter = first_pass.into_iter();
        let mut fallback_run = String::new();
        let mut fallback_start_pos = 0;
        while let Some(item) = item_iter.next() {
            match item {
                MaybeShaped::Resolved(info) => {
                    results.push(info);
                }
                MaybeShaped::Unresolved { raw, slice_start } => {
                    // There was no glyph in that font, so we'll need to shape
                    // using a fallback.  Let's collect together any potential
                    // run of unresolved entries first
                    fallback_start_pos = slice_start;
                    for &c in &raw.unicodes {
                        fallback_run.push(c);
                    }

                    // Clippy can't see that we're nested
                    #[allow(clippy::while_let_on_iterator)]
                    while let Some(item) = item_iter.next() {
                        match item {
                            MaybeShaped::Unresolved { raw, .. } => {
                                for &c in &raw.unicodes {
                                    fallback_run.push(c);
                                }
                            }
                            MaybeShaped::Resolved(info) => {
                                self.shape_into(
                                    font_index + 1,
                                    &fallback_run,
                                    fallback_start_pos + slice_index,
                                    script,
                                    lang,
                                    font_size,
                                    dpi,
                                    results,
                                )?;
                                fallback_run.clear();
                                results.push(info);
                                break;
                            }
                        }
                    }
                }
            }
        }

        if !fallback_run.is_empty() {
            self.shape_into(
                font_index + 1,
                &fallback_run,
                fallback_start_pos + slice_index,
                script,
                lang,
                font_size,
                dpi,
                results,
            )?;
        }

        Ok(())
    }
}

impl FontShaper for AllsortsShaper {
    fn shape(&self, text: &str, size: f64, dpi: u32) -> anyhow::Result<Vec<GlyphInfo>> {
        let mut results = vec![];
        let script = allsorts::tag::LATN;
        let lang = allsorts::tag::DFLT;
        self.shape_into(0, text, 0, script, lang, size, dpi, &mut results)?;
        // log::error!("shape {} into {:?}", text, results);
        Ok(results)
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
