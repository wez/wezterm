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

use font::ft::ftwrap;
use font::ft::hbwrap;
use std::mem;
use std::slice;

struct Glyph<'a> {
    tex: Texture<'a>,
    width: u32,
    height: u32,
    x_advance: i32,
    y_advance: i32,
    x_offset: i32,
    y_offset: i32,
    bearing_x: i32,
    bearing_y: i32,
}

impl<'a> Glyph<'a> {
    fn new<T>(
        texture_creator: &'a TextureCreator<T>,
        glyph: &ftwrap::FT_GlyphSlotRec_,
        pos: &hbwrap::hb_glyph_position_t,
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
                    x_advance: pos.x_advance / 64,
                    y_advance: pos.y_advance / 64,
                    x_offset: pos.x_offset / 64,
                    y_offset: pos.y_offset / 64,
                    bearing_x: glyph.bitmap_left,
                    bearing_y: glyph.bitmap_top,
                })
            }
            mode @ _ => bail!("unhandled pixel mode: {:?}", mode),
        }
    }
}

fn glyphs_for_text<'a, T>(
    texture_creator: &'a TextureCreator<T>,
    s: &str,
) -> Result<Vec<Glyph<'a>>, Error> {
    let mut lib = ftwrap::Library::new()?;
    lib.set_lcd_filter(
        ftwrap::FT_LcdFilter::FT_LCD_FILTER_DEFAULT,
    )?;
    let mut face =
        lib.new_face("/home/wez/.fonts/OperatorMonoLig-Book.otf", 0)?;
    face.set_char_size(0, 36 * 64, 96, 96)?;
    let mut font = hbwrap::Font::new(&face);
    let lang = hbwrap::language_from_string("en")?;
    let mut buf = hbwrap::Buffer::new()?;
    buf.set_script(hbwrap::HB_SCRIPT_LATIN);
    buf.set_direction(hbwrap::HB_DIRECTION_LTR);
    buf.set_language(lang);
    buf.add_str(s);
    let features = vec![
        // kerning
        hbwrap::feature_from_string("kern")?,
        // ligatures
        hbwrap::feature_from_string("liga")?,
        // contextual ligatures
        hbwrap::feature_from_string("clig")?,
    ];
    font.shape(&mut buf, Some(features.as_slice()));

    let infos = buf.glyph_infos();
    let positions = buf.glyph_positions();
    let mut result = Vec::new();

    for (i, info) in infos.iter().enumerate() {
        let pos = &positions[i];
        println!(
            "info {} glyph_pos={}, cluster={} x_adv={} y_adv={} x_off={} y_off={}",
            i,
            info.codepoint,
            info.cluster,
            pos.x_advance,
            pos.y_advance,
            pos.x_offset,
            pos.y_offset
        );

        let glyph = face.load_and_render_glyph(
            info.codepoint,
            (ftwrap::FT_LOAD_COLOR) as i32,
            ftwrap::FT_Render_Mode::FT_RENDER_MODE_LCD,
        )?;


        let g = Glyph::new(texture_creator, glyph, pos)?;

        /*
        println!(
            "width={} height={} advx={} advy={} bearing={},{}",
            g.width,
            g.height,
            g.x_advance,
            g.y_advance,
            g.bearing_x,
            g.bearing_y
        ); */

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
    let glyphs = glyphs_for_text(&texture_creator, "foo->bar();")?;

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
                for g in glyphs.iter() {
                    canvas
                        .copy(
                            &g.tex,
                            Some(Rect::new(0, 0, g.width, g.height)),
                            Some(Rect::new(
                                x + g.x_offset - g.bearing_x,
                                y - (g.y_offset + g.bearing_y as i32) as i32,
                                g.width,
                                g.height,
                            )),
                        )
                        .map_err(failure::err_msg)?;
                    x += g.x_advance;
                    y += g.y_advance;
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
