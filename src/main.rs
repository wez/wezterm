#[macro_use]
extern crate failure;
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
extern crate xcb_util;

use std::mem;
use std::slice;

use cairo::XCBSurface;
use cairo::prelude::*;

mod font;
use font::{Font, FontPattern, ftwrap};

fn cairo_err(status: cairo::Status) -> Error {
    format_err!("cairo status: {:?}", status)
}

struct TerminalWindow<'a> {
    window_id: u32,
    screen_num: i32,
    conn: &'a xcb::Connection,
    width: u16,
    height: u16,
    font: Font,
    cell_height: f64,
    cell_width: f64,
    descender: f64,
    cairo_context: cairo::Context,
    window_surface: cairo::Surface,
    buffer_surface: cairo::ImageSurface,
    need_paint: bool,
}

impl<'a> TerminalWindow<'a> {
    fn new(
        conn: &xcb::Connection,
        screen_num: i32,
        width: u16,
        height: u16,
    ) -> Result<TerminalWindow, Error> {

        let mut pattern = FontPattern::parse("Operator Mono SSm:size=16")?;
        pattern.add_double("dpi", 96.0)?;
        let mut font = Font::new(pattern)?;
        // we always load the cell_height for font 0,
        // regardless of which font we are shaping here,
        // so that we can scale glyphs appropriately
        let (cell_height, cell_width, descender) = font.get_metrics()?;

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
                (
                    xcb::CW_EVENT_MASK,
                    xcb::EVENT_MASK_EXPOSURE | xcb::EVENT_MASK_KEY_PRESS | xcb::EVENT_MASK_STRUCTURE_NOTIFY,
                ),
            ],
        );
        xcb_util::icccm::set_wm_name(&conn, window_id, "wterm");

        let window_surface =
            TerminalWindow::make_cairo_surface(&conn, screen_num, window_id, width, height)?;
        let buffer_surface =
            cairo::ImageSurface::create(cairo::Format::ARgb32, width as i32, height as i32)
                .map_err(cairo_err)?;

        let cairo_context = cairo::Context::new(&window_surface);

        let descender = if descender.is_positive() {
            ((descender as f64) / 64.0).ceil()
        } else {
            ((descender as f64) / 64.0).floor()
        };

        Ok(TerminalWindow {
            window_id,
            screen_num,
            conn,
            width,
            height,
            font,
            cell_height,
            cell_width,
            descender,
            cairo_context,
            buffer_surface,
            window_surface,
            need_paint: true,
        })
    }

    fn show(&self) {
        xcb::map_window(self.conn, self.window_id);
    }

    fn make_cairo_surface(
        conn: &xcb::Connection,
        screen_num: i32,
        window_id: u32,
        width: u16,
        height: u16,
    ) -> Result<cairo::Surface, Error> {
        let screen = conn.get_setup().roots().nth(screen_num as usize).ok_or(
            failure::err_msg("no screen?"),
        )?;
        Ok(cairo::Surface::create(
            &cairo::XCBConnection(
                unsafe { mem::transmute(conn.get_raw_conn()) },
            ),
            &cairo::XCBDrawable(window_id),
            &cairo::XCBVisualType(unsafe {
                mem::transmute(&mut visual_for_screen(&screen).base as *mut _)
            }),
            width as i32,
            height as i32,
        ))
    }

    fn resize_surfaces(&mut self, width: u16, height: u16) -> Result<bool, Error> {
        if width != self.width || height != self.height {
            debug!("resize {},{}", width, height);
            self.buffer_surface =
                cairo::ImageSurface::create(cairo::Format::ARgb32, width as i32, height as i32)
                    .map_err(cairo_err)?;

            self.window_surface.set_size(width as i32, height as i32);
            self.width = width;
            self.height = height;
            self.need_paint = true;
            Ok(true)
        } else {
            debug!("ignoring extra resize");
            Ok(false)
        }
    }

    fn expose(&mut self, x: u16, y: u16, width: u16, height: u16) -> Result<(), Error> {
        debug!("expose {},{}, {},{}", x, y, width, height);
        self.cairo_context.reset_clip();
        self.cairo_context.set_source_surface(
            &self.buffer_surface,
            0.0,
            0.0,
        );
        self.cairo_context.rectangle(
            x as f64,
            y as f64,
            width as f64,
            height as f64,
        );
        self.cairo_context.clip();
        self.cairo_context.paint();

        self.conn.flush();

        Ok(())
    }

    fn paint(&mut self) -> Result<(), Error> {
        debug!("paint");
        self.need_paint = false;
        let ctx = cairo::Context::new(&self.buffer_surface);

        let message = "x_advance != foo->bar(); ❤ 😍🤢";

        ctx.set_source_rgb(0.0, 0.0, 0.0);
        ctx.paint();

        let mut x = 0.0;
        let mut y = self.cell_height.ceil();
        let glyph_info = self.font.shape(0, message)?;
        for info in glyph_info {
            let has_color = self.font.has_color(info.font_idx)?;
            let ft_glyph = self.font.load_glyph(info.font_idx, info.glyph_pos)?;

            let scale = if (info.x_advance / info.num_cells as f64).floor() > self.cell_width {
                info.num_cells as f64 * (self.cell_width / info.x_advance)
            } else if ft_glyph.bitmap.rows as f64 > self.cell_height {
                self.cell_height / ft_glyph.bitmap.rows as f64
            } else {
                1.0f64
            };

            if ft_glyph.bitmap.width == 0 || ft_glyph.bitmap.rows == 0 {
                // a whitespace glyph
            } else {

                let mode: ftwrap::FT_Pixel_Mode =
                    unsafe { mem::transmute(ft_glyph.bitmap.pixel_mode as u32) };

                // pitch is the number of bytes per source row
                let pitch = ft_glyph.bitmap.pitch.abs() as usize;
                let data = unsafe {
                    slice::from_raw_parts_mut(
                        ft_glyph.bitmap.buffer,
                        ft_glyph.bitmap.rows as usize * pitch,
                    )
                };

                let cairo_surface = match mode {
                    ftwrap::FT_Pixel_Mode::FT_PIXEL_MODE_LCD => {
                        // The FT rasterization is often not aligned in the way that
                        // cairo would like, so let's allocate a surface of the correct
                        // size and fill that up.
                        let mut surface = cairo::ImageSurface::create(
                            cairo::Format::Rgb24,
                            (ft_glyph.bitmap.width / 3) as i32,
                            ft_glyph.bitmap.rows as i32,
                        ).map_err(cairo_err)?;
                        {
                            let dest_pitch = surface.get_stride() as usize;
                            let mut dest_data = surface.get_data()?;
                            for y in 0..ft_glyph.bitmap.rows as usize {
                                let dest_offset = y * dest_pitch;
                                let src_offset = y * pitch;
                                dest_data[dest_offset..dest_offset + pitch]
                                    .copy_from_slice(&data[src_offset..src_offset + pitch]);
                            }
                        }
                        Ok(surface)
                    }
                    ftwrap::FT_Pixel_Mode::FT_PIXEL_MODE_GRAY => {
                        let mut surface = cairo::ImageSurface::create(
                            cairo::Format::Rgb24,
                            ft_glyph.bitmap.width as i32,
                            ft_glyph.bitmap.rows as i32,
                        ).map_err(cairo_err)?;
                        {
                            let dest_pitch = surface.get_stride() as usize;
                            let mut dest_data = surface.get_data()?;
                            for y in 0..ft_glyph.bitmap.rows as usize {
                                let src_offset = y * pitch;
                                for x in 0..ft_glyph.bitmap.width as usize {
                                    let dest_offset = (y * dest_pitch) + (x * 3);
                                    let gray = data[src_offset + x];
                                    dest_data[dest_offset + 0] = gray;
                                    dest_data[dest_offset + 1] = gray;
                                    dest_data[dest_offset + 2] = gray;
                                }
                            }
                        }
                        Ok(surface)
                    }
                    ftwrap::FT_Pixel_Mode::FT_PIXEL_MODE_BGRA => unsafe {
                        cairo::ImageSurface::from_raw_full(
                            cairo_sys::cairo_image_surface_create_for_data(
                                data.as_mut_ptr(),
                                cairo::Format::ARgb32,
                                ft_glyph.bitmap.width as i32,
                                ft_glyph.bitmap.rows as i32,
                                pitch as i32,
                            ),
                        )
                    },
                    mode @ _ => bail!("unhandled pixel mode: {:?}", mode),
                }.map_err(cairo_err)?;

                let bearing_x = ft_glyph.bitmap_left as f64;
                let bearing_y = ft_glyph.bitmap_top as f64;

                debug!(
                    "x,y: {},{} desc={} bearing:{},{} off={},{} adv={},{} scale={} width={}",
                    x,
                    y,
                    self.descender,
                    bearing_x,
                    bearing_y,
                    info.x_offset,
                    info.y_offset,
                    info.x_advance,
                    info.y_advance,
                    scale,
                    cairo_surface.get_width(),
                );
                ctx.translate(x, y + self.descender);
                ctx.scale(scale, scale);

                ctx.set_source_surface(
                    &cairo_surface,
                    // Destination for the paint operation
                    info.x_offset + bearing_x,
                    -(info.y_offset + bearing_y),
                );
                ctx.paint();

                if !has_color || false {
                    // Apply text color.
                    // TODO: we only do this for non-colored fonts, but
                    // we could apply it to those also if the current
                    // cell color is not the default foreground attribute.
                    ctx.save();
                    ctx.rectangle(
                        info.x_offset + bearing_x,
                        -(info.y_offset + bearing_y),
                        cairo_surface.get_width() as f64,
                        cairo_surface.get_height() as f64,
                    );
                    ctx.clip();
                    ctx.set_source_rgb(0.7, 0.7, 0.7);
                    ctx.set_operator(cairo::Operator::Multiply);
                    ctx.paint();
                    ctx.restore();
                }

                ctx.identity_matrix();
            }

            // for debugging purposes, outline the cell
            ctx.set_source_rgb(0.2, 0.2, 0.2);
            ctx.rectangle(x, y - self.cell_height, self.cell_width, self.cell_height);
            ctx.stroke();

            x += scale * info.x_advance;
            y += scale * info.y_advance;
        }

        let width = self.width;
        let height = self.height;
        self.expose(0, 0, width, height)?;

        Ok(())
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

fn run() -> Result<(), Error> {
    let (conn, screen_num) = xcb::Connection::connect(None)?;
    println!("Connected screen {}", screen_num);

    let mut window = TerminalWindow::new(&conn, screen_num, 1024, 300)?;
    window.show();
    conn.flush();

    loop {
        // If we need to re-render the display, try to defer that until after we've
        // consumed the input queue
        let event = if window.need_paint {
            match conn.poll_for_queued_event() {
                None => {
                    window.paint()?;
                    continue;
                }
                Some(event) => Some(event),
            }
        } else {
            conn.wait_for_event()
        };
        match event {
            None => break,
            Some(event) => {
                let r = event.response_type() & 0x7f;
                match r {
                    xcb::EXPOSE => {
                        let expose: &xcb::ExposeEvent = unsafe { xcb::cast_event(&event) };
                        window.expose(
                            expose.x(),
                            expose.y(),
                            expose.width(),
                            expose.height(),
                        )?;
                    }
                    xcb::CONFIGURE_NOTIFY => {
                        let cfg: &xcb::ConfigureNotifyEvent = unsafe { xcb::cast_event(&event) };
                        window.resize_surfaces(cfg.width(), cfg.height())?;
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
    run().unwrap();
}
