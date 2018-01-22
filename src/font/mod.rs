use failure::{self, Error};
use std::slice;
use unicode_width::UnicodeWidthStr;

pub mod ftwrap;
pub mod hbwrap;
pub mod fcwrap;
pub mod cairo;

pub use self::fcwrap::Pattern as FontPattern;

/// Holds information about a shaped glyph
#[derive(Clone, Debug)]
pub struct GlyphInfo {
    /// We only retain text in debug mode for diagnostic purposes
    #[cfg(debug_assertions)]
    pub text: String,
    /// Offset within text
    #[cfg(debug_assertions)]
    pub cluster: u32,
    /// How many cells/columns this glyph occupies horizontally
    pub num_cells: u8,
    /// Which font alternative to use; index into Font.fonts
    pub font_idx: usize,
    /// Which freetype glyph to load
    pub glyph_pos: u32,
    /// How far to advance the render cursor after drawing this glyph
    pub x_advance: f64,
    /// How far to advance the render cursor after drawing this glyph
    pub y_advance: f64,
    /// Destination render offset
    pub x_offset: f64,
    /// Destination render offset
    pub y_offset: f64,
}

impl GlyphInfo {
    pub fn new(
        text: &str,
        font_idx: usize,
        info: &hbwrap::hb_glyph_info_t,
        pos: &hbwrap::hb_glyph_position_t,
    ) -> GlyphInfo {
        let num_cells = UnicodeWidthStr::width(text) as u8;
        GlyphInfo {
            #[cfg(debug_assertions)]
            text: text.into(),
            num_cells,
            font_idx,
            glyph_pos: info.codepoint,
            #[cfg(debug_assertions)]
            cluster: info.cluster,
            x_advance: pos.x_advance as f64 / 64.0,
            y_advance: pos.y_advance as f64 / 64.0,
            x_offset: pos.x_offset as f64 / 64.0,
            y_offset: pos.y_offset as f64 / 64.0,
        }
    }
}

/// Holds a loaded font alternative
struct FontInfo {
    face: ftwrap::Face,
    font: hbwrap::Font,
    cairo_face: cairo::FontFace,
    /// nominal monospace cell height
    cell_height: f64,
    /// nominal monospace cell width
    cell_width: f64,
}

/// Holds "the" font selected by the user.  In actuality, it
/// holds the set of fallback fonts that match their criteria
pub struct Font {
    lib: ftwrap::Library,
    pattern: fcwrap::Pattern,
    font_list: fcwrap::FontSet,
    fonts: Vec<FontInfo>,
}

impl Drop for Font {
    fn drop(&mut self) {
        // Ensure that we drop the fonts before we drop the
        // library, otherwise we will end up faulting
        self.fonts.clear();
    }
}

impl Font {
    /// Construct a new Font from the user supplied pattern
    pub fn new(mut pattern: FontPattern) -> Result<Font, Error> {
        let mut lib = ftwrap::Library::new()?;
        lib.set_lcd_filter(
            ftwrap::FT_LcdFilter::FT_LCD_FILTER_DEFAULT,
        )?;

        // Enable some filtering options and pull in the standard
        // fallback font selection from the user configuration
        pattern.monospace()?;
        pattern.config_substitute(fcwrap::MatchKind::Pattern)?;
        pattern.default_substitute();

        // and obtain the selection with the best preference
        // at index 0.
        let font_list = pattern.sort(true)?;

        Ok(Font {
            lib,
            font_list,
            pattern,
            fonts: Vec::new(),
        })
    }

    fn load_next_fallback(&mut self) -> Result<(), Error> {
        let idx = self.fonts.len();
        let pat = self.font_list.iter().nth(idx).ok_or(failure::err_msg(
            "no more fallbacks",
        ))?;
        let pat = self.pattern.render_prepare(&pat)?;
        let file = pat.get_file()?;

        debug!("load_next_fallback: file={}", file);
        debug!("{}", pat.format("%{=unparse}")?);

        let size = pat.get_double("size")?;
        let dpi = pat.get_double("dpi")? as u32;
        debug!("set_char_size {} dpi={}", size, dpi);
        // Scaling before truncating to integer minimizes the chances of hitting
        // the fallback code for set_pixel_sizes below.
        let size = (size * 64.0) as i64;

        let mut face = self.lib.new_face(file, 0)?;

        match face.set_char_size(size, size, dpi, dpi) {
            Err(err) => {
                let sizes = unsafe {
                    let rec = &(*face.face);
                    slice::from_raw_parts(rec.available_sizes, rec.num_fixed_sizes as usize)
                };
                if sizes.len() == 0 {
                    return Err(err);
                } else {
                    // Find the best matching size.
                    // We just take the biggest.
                    let mut size = 0i16;
                    for info in sizes.iter() {
                        size = size.max(info.height);
                    }
                    face.set_pixel_sizes(size as u32, size as u32)?;
                    debug!("fall back to set_pixel_sizes {}", size);
                }
            }
            Ok(_) => {}
        }
        let font = hbwrap::Font::new(&face);

        // Compute metrics for the nominal monospace cell
        let (cell_width, cell_height) = face.cell_metrics();
        debug!("metrics: width={} height={}", cell_width, cell_height);

        let cairo_face = face.as_cairo();
        self.fonts.push(FontInfo {
            face,
            font,
            cairo_face,
            cell_height,
            cell_width,
        });
        Ok(())
    }

    pub fn get_cairo_font(&mut self, idx: usize) -> Result<cairo::FontFace, Error> {
        let font = self.get_font(idx)?;
        Ok(font.cairo_face.clone())
    }

    fn get_font(&mut self, idx: usize) -> Result<&mut FontInfo, Error> {
        if idx >= self.fonts.len() {
            self.load_next_fallback()?;
            ensure!(
                idx < self.fonts.len(),
                "should not ask for a font later than the next prepared font"
            );
        }

        Ok(&mut self.fonts[idx])
    }

    pub fn has_color(&mut self, idx: usize) -> Result<bool, Error> {
        let font = self.get_font(idx)?;
        unsafe {
            Ok(
                ((*font.face.face).face_flags & ftwrap::FT_FACE_FLAG_COLOR as i64) != 0,
            )
        }
    }

    pub fn get_metrics(&mut self) -> Result<(f64, f64, i16), Error> {
        let font = self.get_font(0)?;
        Ok((font.cell_height, font.cell_width, unsafe {
            (*font.face.face).descender
        }))
    }

    pub fn shape(&mut self, font_idx: usize, s: &str) -> Result<Vec<GlyphInfo>, Error> {
        debug!(
            "shape text for font_idx {} with len {} {}",
            font_idx,
            s.len(),
            s
        );
        let features = vec![
            // kerning
            hbwrap::feature_from_string("kern")?,
            // ligatures
            hbwrap::feature_from_string("liga")?,
            // contextual ligatures
            hbwrap::feature_from_string("clig")?,
        ];

        let mut buf = hbwrap::Buffer::new()?;
        buf.set_script(hbwrap::HB_SCRIPT_LATIN);
        buf.set_direction(hbwrap::HB_DIRECTION_LTR);
        buf.set_language(hbwrap::language_from_string("en")?);
        buf.add_str(s);

        self.shape_with_font(font_idx, &mut buf, &features)?;
        let infos = buf.glyph_infos();
        let positions = buf.glyph_positions();

        let mut cluster = Vec::new();

        let mut last_text_pos = None;
        let mut first_fallback_pos = None;

        // Compute the lengths of the text clusters.
        // Ligatures and combining characters mean
        // that a single glyph can take the place of
        // multiple characters.  The 'cluster' member
        // of the glyph info is set to the position
        // in the input utf8 text, so we make a pass
        // over the set of clusters to look for differences
        // greater than 1 and backfill the length of
        // the corresponding text fragment.  We need
        // the fragments to properly handle fallback,
        // and they're handy to have for debugging
        // purposes too.
        let mut sizes = Vec::with_capacity(s.len());
        for (i, info) in infos.iter().enumerate() {
            let pos = info.cluster as usize;
            let mut size = 1;
            if let Some(last_pos) = last_text_pos {
                let diff = pos - last_pos;
                if diff > 1 {
                    sizes[i - 1] = diff;
                }
            } else if pos != 0 {
                size = pos;
            }
            last_text_pos = Some(pos);
            sizes.push(size);
        }
        if let Some(last_pos) = last_text_pos {
            let diff = s.len() - last_pos;
            if diff > 1 {
                let last = sizes.len() - 1;
                sizes[last] = diff;
            }
        }
        debug!("sizes: {:?}", sizes);

        // Now make a second pass to determine if we need
        // to perform fallback to a later font.
        // We can determine this by looking at the codepoint.
        for (i, info) in infos.iter().enumerate() {
            let pos = info.cluster as usize;
            if info.codepoint == 0 {
                if first_fallback_pos.is_none() {
                    // Start of a run that needs fallback
                    first_fallback_pos = Some(pos);
                }
            } else if let Some(start) = first_fallback_pos {
                // End of a fallback run
                debug!("range: {:?}-{:?} needs fallback", start, pos);

                let substr = &s[start..pos];
                let mut shape = self.shape(font_idx + 1, substr)?;
                cluster.append(&mut shape);

                first_fallback_pos = None;
            }
            if info.codepoint != 0 {
                let text = &s[pos..pos + sizes[i]];
                //debug!("glyph from `{}`", text);
                cluster.push(GlyphInfo::new(text, font_idx, info, &positions[i]));
            }
        }

        // Check to see if we started and didn't finish a
        // fallback run.
        if let Some(start) = first_fallback_pos {
            let substr = &s[start..];
            debug!(
                "at end {:?}-{:?} needs fallback {}",
                start,
                s.len() - 1,
                substr,
            );
            let mut shape = self.shape(font_idx + 1, substr)?;
            cluster.append(&mut shape);
        }

        //debug!("shaped: {:#?}", cluster);

        Ok(cluster)
    }

    fn shape_with_font(
        &mut self,
        idx: usize,
        buf: &mut hbwrap::Buffer,
        features: &Vec<hbwrap::hb_feature_t>,
    ) -> Result<(), Error> {
        let info = self.get_font(idx)?;
        info.font.shape(buf, Some(features.as_slice()));
        Ok(())
    }

    pub fn load_glyph(
        &mut self,
        font_idx: usize,
        glyph_pos: u32,
    ) -> Result<&ftwrap::FT_GlyphSlotRec_, Error> {
        let info = &mut self.fonts[font_idx];

        let render_mode =//ftwrap::FT_Render_Mode::FT_RENDER_MODE_NORMAL;
            ftwrap::FT_Render_Mode::FT_RENDER_MODE_LCD;

        // when changing the load flags, we also need
        // to change them for harfbuzz otherwise it won't
        // hint correctly
        let load_flags = (ftwrap::FT_LOAD_COLOR) as i32 |
            // enable FT_LOAD_TARGET bits.  There are no flags defined
            // for these in the bindings so we do some bit magic for
            // ourselves.  This is how the FT_LOAD_TARGET_() macro
            // assembles these bits.
            (render_mode as i32) << 16;

        info.font.set_load_flags(load_flags);
        info.face.load_and_render_glyph(
            glyph_pos,
            load_flags,
            render_mode,
        )
    }
}
