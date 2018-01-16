#[macro_use]
extern crate failure;
extern crate hexdump;

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

#[derive(Copy, Clone, Debug)]
struct GlyphInfo {
    glyph_pos: u32,
    cluster: u32,
    x_advance: i32,
    y_advance: i32,
    x_offset: i32,
    y_offset: i32,
}

impl GlyphInfo {
    pub fn new(
        info: &hbwrap::hb_glyph_info_t,
        pos: &hbwrap::hb_glyph_position_t,
    ) -> GlyphInfo {
        GlyphInfo {
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
    tex: Texture<'a>,
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
    ) -> Result<Glyph<'a>, Error> {
        let mode: ftwrap::FT_Pixel_Mode =
            unsafe { mem::transmute(glyph.bitmap.pixel_mode as u32) };

        // pitch is the number of bytes per source row
        let pitch = glyph.bitmap.pitch.abs() as usize;

        match mode {
            ftwrap::FT_Pixel_Mode::FT_PIXEL_MODE_LCD => {
                let width = (glyph.bitmap.width as usize) / 3;
                let height = glyph.bitmap.rows as usize;
                let mut tex = texture_creator
                    .create_texture_static(
                        Some(PixelFormatEnum::BGR24),
                        width as u32,
                        height as u32,
                    )
                    .map_err(failure::err_msg)?;
                let data = unsafe {
                    slice::from_raw_parts(glyph.bitmap.buffer, height * pitch)
                };
                tex.update(None, data, pitch)?;

                Ok(Glyph {
                    tex,
                    width: width as u32,
                    height: height as u32,
                    info: *info,
                    bearing_x: glyph.bitmap_left,
                    bearing_y: glyph.bitmap_top,
                })
            }
            mode @ _ => bail!("unhandled pixel mode: {:?}", mode),
        }
    }
}

struct FontInfo {
    face: ftwrap::Face,
    font: hbwrap::Font,
}

struct FontHolder {
    lib: ftwrap::Library,
    size: i64,
    pattern: fcwrap::Pattern,
    font_list: fcwrap::FontSet,
    fonts: Vec<FontInfo>,
}

#[derive(Debug)]
struct ShapedCluster {
    /// index into FontHolder.fonts
    font_idx: usize,
    /// holds shaped results
    info: Vec<GlyphInfo>,
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

        let mut pattern = fcwrap::Pattern::new()?;
        pattern.family("Operator Mono SSm Lig")?;
        pattern.family("Emoji One")?;
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

        let mut face = self.lib.new_face(file, 0)?;
        face.set_char_size(0, self.size * 64, 96, 96)?;
        let font = hbwrap::Font::new(&face);

        self.fonts.push(FontInfo { face, font });
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

    fn shape(&mut self, s: &str) -> Result<Vec<ShapedCluster>, Error> {
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

        let font_idx = 0;

        self.shape_with_font(font_idx, &mut buf, &features)?;
        let infos = buf.glyph_infos();
        let positions = buf.glyph_positions();

        let mut cluster = Vec::new();

        for (i, info) in infos.iter().enumerate() {
            let pos = &positions[i];
            // TODO: if info.codepoint == 0 here then we should
            // rebuild that portion of the string and reshape it
            // with the next fallback font
            cluster.push(GlyphInfo::new(info, pos));
        }

        println!("shaped: {:?}", cluster);

        Ok(vec![
            ShapedCluster {
                font_idx,
                info: cluster,
            },
        ])
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

    let mut font_holder = FontHolder::new(36)?;

    let mut result = Vec::new();
    for cluster in font_holder.shape(s)? {
        for info in cluster.info.iter() {
            if info.glyph_pos == 0 {
                println!("skip: no codepoint for this one");
                continue;
            }

            let glyph =
                font_holder.load_glyph(cluster.font_idx, info.glyph_pos)?;

            if glyph.bitmap.width == 0 || glyph.bitmap.rows == 0 {
                println!("skip: bitmap for this has 0 dimensions {:?}", glyph);
                continue;
            }

            let g = Glyph::new(texture_creator, glyph, info)?;
            result.push(g);
        }
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
    let mut glyphs = glyphs_for_text(&texture_creator, "foo->bar(); ❤")?;

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
                    g.tex.set_color_mod(0xb3, 0xb3, 0xb3);
                    canvas
                        .copy(
                            &g.tex,
                            Some(Rect::new(0, 0, g.width, g.height)),
                            Some(Rect::new(
                                x + g.info.x_offset - g.bearing_x,
                                y - (g.info.y_offset + g.bearing_y as i32) as i32,
                                g.width,
                                g.height,
                            )),
                        )
                        .map_err(failure::err_msg)?;
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
