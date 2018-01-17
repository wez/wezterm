#[macro_use]
extern crate failure;
extern crate hexdump;
extern crate unicode_width;
extern crate harfbuzz_sys;
#[cfg(not(target_os = "macos"))]
extern crate fontconfig; // from servo-fontconfig
#[cfg(not(target_os = "macos"))]
extern crate freetype;

mod font;

use failure::Error;
extern crate sdl2;
use sdl2::event::{Event, WindowEvent};
use sdl2::keyboard::Keycode;
use sdl2::pixels::{Color, PixelFormatEnum};
use sdl2::rect::Rect;
use sdl2::render::{BlendMode, Texture, TextureCreator};

use font::ftwrap;
use std::mem;
use std::slice;

use font::{Font, FontPattern, GlyphInfo};

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
        let mut has_color = false;

        if width == 0 || height == 0 {
            // Special case for zero sized bitmaps; we can't
            // build a texture with zero dimenions, so we return
            // a Glyph with tex=None.
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
                    has_color = true;
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
            has_color,
            tex,
            width,
            height,
            info,
            bearing_x,
            bearing_y,
        })
    }
}

fn glyphs_for_text<'a, T>(
    texture_creator: &'a TextureCreator<T>,
    s: &str,
) -> Result<Vec<Glyph<'a>>, Error> {

    let pattern = FontPattern::parse("Operator Mono SSm:size=12:weight=SemiLight")?;
    let mut font = Font::new(pattern)?;

    // We always load the cell_height for font 0,
    // regardless of which font we are shaping here,
    // so that we can scale glyphs appropriately
    let (cell_height, cell_width) = font.get_metrics()?;

    let mut result = Vec::new();
    for info in font.shape(0, s)? {
        if info.glyph_pos == 0 {
            println!("skip: no codepoint for this one {:?}", info);
            continue;
        }

        let glyph = font.load_glyph(info.font_idx, info.glyph_pos)?;

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
