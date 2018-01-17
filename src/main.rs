#[macro_use]
extern crate failure;
extern crate hexdump;
extern crate unicode_width;

use unicode_width::UnicodeWidthStr;

use failure::Error;
extern crate font;
extern crate sdl2;
use sdl2::event::{Event, WindowEvent};
use sdl2::keyboard::Keycode;
use sdl2::pixels::{Color, PixelFormatEnum};
use sdl2::rect::Rect;
use sdl2::render::{BlendMode, Texture, TextureCreator};

use font::ft::fcwrap;
use font::ft::ftwrap;
use font::ft::hbwrap;
use std::mem;
use std::slice;

#[derive(Clone, Debug)]
struct GlyphInfo {
    /// We only retain text in debug mode for diagnostic purposes
    #[cfg(debug_assertions)]
    text: String,
    num_cells: u8,
    font_idx: usize,
    glyph_pos: u32,
    cluster: u32,
    x_advance: i32,
    y_advance: i32,
    x_offset: i32,
    y_offset: i32,
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
            cluster: info.cluster,
            x_advance: pos.x_advance / 64,
            y_advance: pos.y_advance / 64,
            x_offset: pos.x_offset / 64,
            y_offset: pos.y_offset / 64,
        }
    }
}


struct Glyph<'a> {
    has_color: bool,
    tex: Option<Texture<'a>>,
    width: u32,
    height: u32,
    info: GlyphInfo,
    bearing_x: i32,
    bearing_y: i32,
}

impl<'a> Glyph<'a> {
    fn new<T>(
        texture_creator: &'a TextureCreator<T>,
        glyph: &ftwrap::FT_GlyphSlotRec_,
        info: &GlyphInfo,
        target_cell_height: i64,
        target_cell_width: i64,
    ) -> Result<Glyph<'a>, Error> {

        let mut info = info.clone();
        let mut tex = None;
        let mut width = glyph.bitmap.width as u32;
        let mut height = glyph.bitmap.rows as u32;
        let has_color = false;

        if width == 0 || height == 0 {
            // Special case for zero sized bitmaps; we can't
            // build a texture with zero dimenions, so we return
            // a Glyph with tex=None.
            tex = None;
        } else {

            let mode: ftwrap::FT_Pixel_Mode =
                unsafe { mem::transmute(glyph.bitmap.pixel_mode as u32) };

            // pitch is the number of bytes per source row
            let pitch = glyph.bitmap.pitch.abs() as usize;
            let data =
                unsafe { slice::from_raw_parts(glyph.bitmap.buffer, height as usize * pitch) };

            let mut t = match mode {
                ftwrap::FT_Pixel_Mode::FT_PIXEL_MODE_LCD => {
                    width = width / 3;
                    texture_creator
                        .create_texture_static(Some(PixelFormatEnum::BGR24), width, height)
                        .map_err(failure::err_msg)?
                }
                ftwrap::FT_Pixel_Mode::FT_PIXEL_MODE_BGRA => {
                    texture_creator
                        .create_texture_static(
                            // Note: the pixelformat is BGRA and we really
                            // want to use BGRA32 as the PixelFormatEnum
                            // value (which is endian corrected) but that
                            // is missing.  Instead, we have to go to the
                            // lower level pixel format value and handle
                            // the endianness for ourselves
                            Some(if cfg!(target_endian = "big") {
                                PixelFormatEnum::BGRA8888
                            } else {
                                PixelFormatEnum::ARGB8888
                            }),
                            width,
                            height,
                        )
                        .map_err(failure::err_msg)?
                }
                mode @ _ => bail!("unhandled pixel mode: {:?}", mode),
            };

            t.update(None, data, pitch)?;
            tex = Some(t);
        }

        let mut bearing_x = glyph.bitmap_left;
        let mut bearing_y = glyph.bitmap_top;

        let scale = if (info.x_advance / info.num_cells as i32) as i64 > target_cell_width {
            info.num_cells as f64 * (target_cell_width as f64 / info.x_advance as f64)
        } else if height as i64 > target_cell_height {
            target_cell_height as f64 / height as f64
        } else {
            1.0f64
        };

        if scale != 1.0f64 {
            println!(
                "scaling {:?} w={} {}, h={} {} by {}",
                info,
                width,
                target_cell_width,
                height,
                target_cell_height,
                scale
            );
            width = (width as f64 * scale) as u32;
            height = (height as f64 * scale) as u32;
            bearing_x = (bearing_x as f64 * scale) as i32;
            bearing_y = (bearing_y as f64 * scale) as i32;
            info.x_advance = (info.x_advance as f64 * scale) as i32;
            info.y_advance = (info.y_advance as f64 * scale) as i32;
            info.x_offset = (info.x_offset as f64 * scale) as i32;
            info.y_offset = (info.y_offset as f64 * scale) as i32;
        }

        Ok(Glyph {
            has_color: false,
            tex: tex,
            width,
            height,
            info,
            bearing_x,
            bearing_y,
        })
    }
}

struct FontInfo {
    face: ftwrap::Face,
    font: hbwrap::Font,
    cell_height: i64,
    cell_width: i64,
}

struct FontHolder {
    lib: ftwrap::Library,
    size: i64,
    pattern: fcwrap::Pattern,
    font_list: fcwrap::FontSet,
    fonts: Vec<FontInfo>,
}

impl Drop for FontHolder {
    fn drop(&mut self) {
        // Ensure that we drop the fonts before we drop the
        // library, otherwise we will end up faulting
        self.fonts.clear();
    }
}

impl FontHolder {
    fn new(size: i64) -> Result<FontHolder, Error> {
        let mut lib = ftwrap::Library::new()?;
        lib.set_lcd_filter(
            ftwrap::FT_LcdFilter::FT_LCD_FILTER_DEFAULT,
        )?;

        let mut pattern = fcwrap::Pattern::parse("Operator Mono SSm:size=12:weight=SemiLight")?;
        //pattern.family("Operator Mono SSm")?;
        pattern.monospace()?;
        pattern.config_substitute(fcwrap::MatchKind::Pattern)?;
        pattern.default_substitute();
        let font_list = pattern.sort(true)?;

        Ok(FontHolder {
            lib,
            size,
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

        println!("load_next_fallback: file={}", file);

        let mut face = self.lib.new_face(file, 0)?;
        match face.set_char_size(0, self.size * 64, 96, 96) {
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
                }
            }
            Ok(_) => {}
        }
        let font = hbwrap::Font::new(&face);

        // Compute metrics for the nominal monospace cell
        let (cell_width, cell_height) = face.cell_metrics();
        println!("metrics: width={} height={}", cell_width, cell_height);

        self.fonts.push(FontInfo {
            face,
            font,
            cell_height,
            cell_width,
        });
        Ok(())
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

    fn shape(&mut self, font_idx: usize, s: &str) -> Result<Vec<GlyphInfo>, Error> {
        println!(
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
        let mut sizes = Vec::new();
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
        println!("sizes: {:?}", sizes);

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
                println!("range: {:?}-{:?} needs fallback", start, pos);

                let substr = &s[start..pos];
                let mut shape = self.shape(font_idx + 1, substr)?;
                cluster.append(&mut shape);

                first_fallback_pos = None;
            }
            if info.codepoint != 0 {
                let text = &s[pos..pos + sizes[i]];
                println!("glyph from `{}`", text);
                cluster.push(GlyphInfo::new(text, font_idx, info, &positions[i]));
            }
        }

        // Check to see if we started and didn't finish a
        // fallback run.
        if let Some(start) = first_fallback_pos {
            let substr = &s[start..];
            println!(
                "at end {:?}-{:?} needs fallback {}",
                start,
                s.len() - 1,
                substr,
            );
            let mut shape = self.shape(font_idx + 1, substr)?;
            cluster.append(&mut shape);
        }

        println!("shaped: {:#?}", cluster);

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

    fn load_glyph(
        &mut self,
        font_idx: usize,
        glyph_pos: u32,
    ) -> Result<&ftwrap::FT_GlyphSlotRec_, Error> {
        let info = &mut self.fonts[font_idx];
        info.face.load_and_render_glyph(
            glyph_pos,
            (ftwrap::FT_LOAD_COLOR) as i32,
            ftwrap::FT_Render_Mode::FT_RENDER_MODE_LCD,
        )
    }
}

fn glyphs_for_text<'a, T>(
    texture_creator: &'a TextureCreator<T>,
    s: &str,
) -> Result<Vec<Glyph<'a>>, Error> {

    let mut font_holder = FontHolder::new(12)?;

    // We always load the cell_height for font 0,
    // regardless of which font we are shaping here,
    // so that we can scale glyphs appropriately
    let (cell_height, cell_width) = {
        let font = font_holder.get_font(0)?;
        (font.cell_height, font.cell_width)
    };

    let mut result = Vec::new();
    for info in font_holder.shape(0, s)? {
        if info.glyph_pos == 0 {
            println!("skip: no codepoint for this one {:?}", info);
            continue;
        }

        let glyph = font_holder.load_glyph(info.font_idx, info.glyph_pos)?;

        let g = Glyph::new(texture_creator, glyph, &info, cell_height, cell_width)?;
        result.push(g);
    }
    Ok(result)
}

fn run() -> Result<(), Error> {
    let sdl_context = sdl2::init().map_err(failure::err_msg)?;
    let video_subsys = sdl_context.video().map_err(failure::err_msg)?;
    let window = video_subsys
        .window("wterm", 1024, 768)
        .resizable()
        .opengl()
        .build()?;
    let mut canvas = window.into_canvas().build()?;
    let texture_creator = canvas.texture_creator();
    let mut glyphs = glyphs_for_text(&texture_creator, "x_advance != foo->bar(); ❤ 😍🤢")?;

    for event in sdl_context
        .event_pump()
        .map_err(failure::err_msg)?
        .wait_iter()
    {
        match event {
            Event::KeyDown { keycode: Some(Keycode::Escape), .. } |
            Event::Quit { .. } => break,
            Event::Window { win_event: WindowEvent::Resized(..), .. } => {
                println!("resize");
            }
            Event::Window { win_event: WindowEvent::Exposed, .. } => {
                println!("exposed");
                canvas.set_draw_color(Color::RGBA(0, 0, 0, 255));
                canvas.clear();
                canvas.set_blend_mode(BlendMode::Blend);

                let mut x = 10i32;
                let mut y = 100i32;
                for g in glyphs.iter_mut() {
                    if let &mut Some(ref mut tex) = &mut g.tex {
                        if !g.has_color {
                            tex.set_color_mod(0xb3, 0xb3, 0xb3);
                        }
                        canvas
                            .copy(
                                &tex,
                                None,
                                Some(Rect::new(
                                    x + g.info.x_offset + g.bearing_x,
                                    y - g.info.y_offset - g.bearing_y,
                                    g.width,
                                    g.height,
                                )),
                            )
                            .map_err(failure::err_msg)?;
                    }
                    x += g.info.x_advance;
                    y += g.info.y_advance;
                }

                canvas.present();
            }
            _ => {}
        }
    }
    Ok(())
}


fn main() {
    run().unwrap();
}
