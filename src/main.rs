#[macro_use]
extern crate failure;
extern crate hexdump;
extern crate unicode_width;
extern crate harfbuzz_sys;
extern crate cairo;
extern crate cairo_sys;
#[cfg(not(target_os = "macos"))]
extern crate fontconfig; // from servo-fontconfig
#[cfg(not(target_os = "macos"))]
extern crate freetype;
#[macro_use]
pub mod log;

use failure::Error;

extern crate xcb;
use std::mem;

/*

extern crate sdl2;
use sdl2::event::{Event, WindowEvent};
use sdl2::keyboard::Keycode;
use sdl2::pixels::{Color, PixelFormatEnum};
use sdl2::rect::Rect;
use sdl2::render::{BlendMode, RenderTarget, Texture, TextureCreator};

use std::slice;

*/

/*
mod font;
use font::{Font, FontPattern, GlyphInfo, ftwrap};
*/

/*
fn cairo_err(status: cairo::Status) -> Error {
    format_err!("cairo status: {:?}", status)
}
*/


/*
struct Glyph<'a> {
    has_color: bool,
    tex: Option<Texture<'a>>,
    width: u32,
    height: u32,
    info: GlyphInfo,
    bearing_x: i32,
    bearing_y: i32,
    cairo_surface: cairo::ImageSurface,
}

// Note: the pixelformat is BGRA and we really
// want to use BGRA32 as the PixelFormatEnum
// value (which is endian corrected) but that
// is missing.  Instead, we have to go to the
// lower level pixel format value and handle
// the endianness for ourselves
#[cfg(target_endian = "big")]
const SDL_BGRA32: PixelFormatEnum = PixelFormatEnum::BGRA8888;
#[cfg(target_endian = "big")]
const SDL_ARGB32: PixelFormatEnum = PixelFormatEnum::ARGB8888;
#[cfg(target_endian = "little")]
const SDL_BGRA32: PixelFormatEnum = PixelFormatEnum::ARGB8888;
#[cfg(target_endian = "little")]
const SDL_ARGB32: PixelFormatEnum = PixelFormatEnum::BGRA8888;

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
                        .create_texture_static(Some(SDL_BGRA32), width, height)
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
            debug!(
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

        let cairo_surface = cairo::ImageSurface::create(
            cairo::Format::ARgb32,
            target_cell_width as i32,
            target_cell_height as i32,
        ).map_err(cairo_err)?;

        let cairo_glyph = cairo::Glyph {
            index: info.glyph_pos as u64,
            x: (0 + info.x_offset + bearing_x) as f64,
            y: (target_cell_height as i32 - info.y_offset - bearing_y) as f64,
        };
        println!("cairo_glyph: {:?}", cairo_glyph);
        let cairo_context = cairo::Context::new(&cairo_surface);
        cairo_context.set_source_rgba(0.0, 0.0, 0.0, 0.0);

        Ok(Glyph {
            has_color,
            tex,
            width,
            height,
            info,
            bearing_x,
            bearing_y,
            cairo_surface,
        })
    }
}

fn glyphs_for_text<'a, T>(
    texture_creator: &'a TextureCreator<T>,
    s: &str,
) -> Result<Vec<Glyph<'a>>, Error> {

    let pattern = FontPattern::parse("Operator Mono SSm Lig:size=12:weight=SemiLight")?;
    let mut font = Font::new(pattern)?;

    // We always load the cell_height for font 0,
    // regardless of which font we are shaping here,
    // so that we can scale glyphs appropriately
    let (cell_height, cell_width) = font.get_metrics()?;

    let mut result = Vec::new();
    for info in font.shape(0, s)? {
        if info.glyph_pos == 0 {
            debug!("skip: no codepoint for this one {:?}", info);
            continue;
        }

        let glyph = font.load_glyph(info.font_idx, info.glyph_pos)?;

        let g = Glyph::new(texture_creator, glyph, &info, cell_height, cell_width)?;
        result.push(g);
    }
    Ok(result)
}

fn with_cairo_surface<T, W: RenderTarget, F: FnMut(cairo::Context)>(
    canvas: &mut sdl2::render::Canvas<W>,
    texture_creator: &TextureCreator<T>,
    mut f: F,
) -> Result<(), Error> {
    let (width, height) = canvas.output_size().map_err(failure::err_msg)?;
    println!("canvas size {} {}", width, height);
    let mut tex = texture_creator.create_texture_streaming(
        Some(PixelFormatEnum::ARGB8888),
        width,
        height,
    )?;
    tex.with_lock(None, |data, pitch| {
        let surface = unsafe {
            assert!(data.len() == pitch * height as usize);
            cairo::ImageSurface::from_raw_full(cairo_sys::cairo_image_surface_create_for_data(
                data.as_mut_ptr(),
                cairo::Format::ARgb32,
                width as i32,
                height as i32,
                pitch as i32,
            ))
        }.expect("failed to create cairo surface");

        let cairo = cairo::Context::new(&surface);
        f(cairo);
    }).map_err(failure::err_msg)?;
    canvas.copy(&tex, None, None).map_err(failure::err_msg)
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
            }
            Event::Window { win_event: WindowEvent::Exposed, .. } => {
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

                with_cairo_surface(&mut canvas, &texture_creator, |cairo| {
                    cairo.set_source_rgb(4.0, 0.2, 0.2);
                    cairo.set_line_width(2.0);
                    cairo.rectangle(200.0, 200.0, 300.0, 300.0);
                    cairo.stroke();
                })?;

                canvas.present();
            }
            _ => {}
        }
    }
    Ok(())
}
*/

struct TerminalWindow<'a> {
    window_id: u32,
    conn: &'a xcb::Connection,
    width: u16,
    height: u16,
}

impl<'a> TerminalWindow<'a> {
    fn new(
        conn: &xcb::Connection,
        screen_num: i32,
        width: u16,
        height: u16,
    ) -> Result<TerminalWindow, Error> {
        let setup = conn.get_setup();
        let screen = setup.roots().nth(screen_num as usize).ok_or(
            failure::err_msg(
                "no screen?",
            ),
        )?;
        let window_id = conn.generate_id();

        xcb::create_window(
            &conn,
            xcb::COPY_FROM_PARENT as u8,
            window_id,
            screen.root(),
            // x, y
            0,
            0,
            // width, height
            width,
            height,
            // border width
            0,
            xcb::WINDOW_CLASS_INPUT_OUTPUT as u16,
            screen.root_visual(),
            &[
                (xcb::CW_BACK_PIXEL, screen.black_pixel()),
                (
                    xcb::CW_EVENT_MASK,
                    xcb::EVENT_MASK_EXPOSURE | xcb::EVENT_MASK_KEY_PRESS | xcb::EVENT_MASK_STRUCTURE_NOTIFY,
                ),
            ],
        );

        Ok(TerminalWindow {
            window_id,
            conn,
            width,
            height,
        })
    }

    fn show(&self) {
        xcb::map_window(self.conn, self.window_id);
    }
}

fn visual_for_screen(screen: &xcb::Screen) -> xcb::Visualtype {
    for depth in screen.allowed_depths() {
        for vis in depth.visuals() {
            if vis.visual_id() == screen.root_visual() {
                return vis;
            }
        }
    }

    unreachable!("screen doesn't have info on its own visual?");
}

fn run_xcb() -> Result<(), Error> {
    use cairo::XCBSurface;
    use cairo::prelude::*;

    let (conn, screen_num) = xcb::Connection::connect(None)?;
    println!("Connected screen {}", screen_num);

    let mut window = TerminalWindow::new(&conn, screen_num, 1024, 768)?;
    window.show();
    conn.flush();

    loop {
        let event = conn.wait_for_event();
        match event {
            None => break,
            Some(event) => {
                let r = event.response_type() & 0x7f;
                match r {
                    xcb::EXPOSE => {
                        println!("expose");
                        let screen = conn.get_setup().roots().nth(screen_num as usize).ok_or(
                            failure::err_msg("no screen?"),
                        )?;
                        let surface =
                            cairo::Surface::create(
                                &cairo::XCBConnection(unsafe { mem::transmute(conn.get_raw_conn()) }),
                                &cairo::XCBDrawable(window.window_id),
                                &cairo::XCBVisualType(unsafe {
                                    mem::transmute(&mut visual_for_screen(&screen).base as *mut _)
                                }),
                                window.width as i32,
                                window.height as i32,
                            );
                        let ctx = cairo::Context::new(&surface);
                        ctx.select_font_face(
                            "Operator Mono",
                            cairo::FontSlant::Normal,
                            cairo::FontWeight::Normal,
                        );
                        ctx.set_font_size(32.0);
                        ctx.set_source_rgb(0.8, 0.8, 0.8);
                        ctx.move_to(10.0, 50.0);
                        ctx.show_text("x_advance != foo->bar(); ❤ 😍🤢");
                        surface.flush();
                        conn.flush();
                    }
                    xcb::CONFIGURE_NOTIFY => {
                        let cfg: &xcb::ConfigureNotifyEvent = unsafe { xcb::cast_event(&event) };
                        window.width = cfg.width();
                        window.height = cfg.height();
                    }
                    xcb::KEY_PRESS => {
                        let key_press: &xcb::KeyPressEvent = unsafe { xcb::cast_event(&event) };
                        debug!("Key '{}' pressed", key_press.detail());
                        break;
                    }
                    _ => {}
                }
            }
        }
    }


    Ok(())
}


fn main() {
    run_xcb().unwrap();
}
