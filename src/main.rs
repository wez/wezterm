#[macro_use]
extern crate failure;
extern crate unicode_width;
extern crate unicode_segmentation;
extern crate harfbuzz_sys;
#[cfg(not(target_os = "macos"))]
extern crate fontconfig; // from servo-fontconfig
#[cfg(not(target_os = "macos"))]
extern crate freetype;
extern crate resize;
extern crate vte;
extern crate libc;
#[macro_use]
pub mod log;

use failure::Error;

extern crate xcb;
extern crate xcb_util;

use std::mem;
use std::slice;

mod xgfx;
mod font;
use font::{Font, FontPattern, ftwrap};

mod term;
mod pty;

struct TerminalWindow<'a> {
    window: xgfx::Window<'a>,
    conn: &'a xcb::Connection,
    width: u16,
    height: u16,
    font: Font,
    cell_height: f64,
    cell_width: f64,
    descender: isize,
    window_context: xgfx::Context<'a>,
    buffer_image: xgfx::Image,
    need_paint: bool,
    terminal: term::Terminal,
}

impl<'a> TerminalWindow<'a> {
    fn new(
        conn: &xcb::Connection,
        screen_num: i32,
        width: u16,
        height: u16,
        terminal: term::Terminal,
    ) -> Result<TerminalWindow, Error> {

        let mut pattern = FontPattern::parse("Operator Mono SSm Lig:size=12")?;
        pattern.add_double("dpi", 96.0)?;
        let mut font = Font::new(pattern)?;
        // we always load the cell_height for font 0,
        // regardless of which font we are shaping here,
        // so that we can scale glyphs appropriately
        let (cell_height, cell_width, descender) = font.get_metrics()?;

        let window = xgfx::Window::new(&conn, screen_num, width, height)?;
        window.set_title("wterm");
        let window_context = xgfx::Context::new(conn, &window);

        let buffer_image = xgfx::Image::new(width as usize, height as usize);

        let descender = if descender.is_positive() {
            ((descender as f64) / 64.0).ceil() as isize
        } else {
            ((descender as f64) / 64.0).floor() as isize
        };

        Ok(TerminalWindow {
            window,
            window_context,
            buffer_image,
            conn,
            width,
            height,
            font,
            cell_height,
            cell_width,
            descender,
            need_paint: true,
            terminal,
        })
    }

    fn show(&self) {
        self.window.show();
    }

    fn resize_surfaces(&mut self, width: u16, height: u16) -> Result<bool, Error> {
        if width != self.width || height != self.height {
            debug!("resize {},{}", width, height);
            let mut buffer = xgfx::Image::new(width as usize, height as usize);
            buffer.draw_image(0, 0, &self.buffer_image, xgfx::Operator::Source);
            self.buffer_image = buffer;
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
        if x == 0 && y == 0 && width == self.width && height == self.height {
            self.window_context.put_image(0, 0, &self.buffer_image);
        } else {
            let mut im = xgfx::Image::new(width as usize, height as usize);
            im.draw_image_subset(
                0,
                0,
                x as usize,
                y as usize,
                width as usize,
                height as usize,
                &self.buffer_image,
                xgfx::Operator::Source,
            );
            self.window_context.put_image(x as i16, y as i16, &im);
        }
        self.conn.flush();

        Ok(())
    }

    fn paint(&mut self) -> Result<(), Error> {
        debug!("paint");
        self.need_paint = false;

        let palette = term::color::ColorPalette::default();
        self.buffer_image.clear(
            palette
                .resolve(&term::color::ColorAttribute::Background)
                .into(),
        );

        let cell_height = self.cell_height.ceil() as usize;
        let mut y = 0 as isize;

        let (phys_cols, lines) = self.terminal.visible_cells();

        for line in lines.iter() {
            let mut x = 0 as isize;
            y += cell_height as isize;

            let glyph_info = self.font.shape(0, &line.as_str())?;
            for (cell_idx, info) in glyph_info.iter().enumerate() {
                let has_color = self.font.has_color(info.font_idx)?;
                let ft_glyph = self.font.load_glyph(info.font_idx, info.glyph_pos)?;

                let attrs = &line.cells[cell_idx].attrs;

                // Render the cell background color
                self.buffer_image.clear_rect(
                    x,
                    y - cell_height as isize,
                    info.num_cells as usize * self.cell_width as usize,
                    cell_height,
                    palette.resolve(&attrs.background).into(),
                );

                let scale = if (info.x_advance / info.num_cells as f64).floor() > self.cell_width {
                    info.num_cells as f64 * (self.cell_width / info.x_advance)
                } else if ft_glyph.bitmap.rows as f64 > self.cell_height {
                    self.cell_height / ft_glyph.bitmap.rows as f64
                } else {
                    1.0f64
                };
                let (x_offset, y_offset, x_advance, y_advance) = if scale != 1.0 {
                    (
                        info.x_offset * scale,
                        info.y_offset * scale,
                        info.x_advance * scale,
                        info.y_advance * scale,
                    )
                } else {
                    (info.x_offset, info.y_offset, info.x_advance, info.y_advance)
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

                    let image = match mode {
                        ftwrap::FT_Pixel_Mode::FT_PIXEL_MODE_LCD => {
                            xgfx::Image::with_bgr24(
                                ft_glyph.bitmap.width as usize / 3,
                                ft_glyph.bitmap.rows as usize,
                                pitch as usize,
                                data,
                            )
                        }
                        ftwrap::FT_Pixel_Mode::FT_PIXEL_MODE_BGRA => {
                            xgfx::Image::with_bgra32(
                                ft_glyph.bitmap.width as usize,
                                ft_glyph.bitmap.rows as usize,
                                pitch as usize,
                                data,
                            )
                        }
                        ftwrap::FT_Pixel_Mode::FT_PIXEL_MODE_GRAY => {
                            xgfx::Image::with_8bpp(
                                ft_glyph.bitmap.width as usize,
                                ft_glyph.bitmap.rows as usize,
                                pitch as usize,
                                data,
                            )
                        }
                        mode @ _ => bail!("unhandled pixel mode: {:?}", mode),
                    };

                    let bearing_x = (ft_glyph.bitmap_left as f64 * scale) as isize;
                    let bearing_y = (ft_glyph.bitmap_top as f64 * scale) as isize;

                    debug!(
                    "x,y: {},{} desc={} bearing:{},{} off={},{} adv={},{} scale={}",
                    x,
                    y,
                    self.descender,
                    bearing_x,
                    bearing_y,
                    x_offset,
                    y_offset,
                    x_advance,
                    y_advance,
                    scale,
                );

                    let image = if scale != 1.0 {
                        image.scale_by(scale)
                    } else {
                        image
                    };

                    let operator = if has_color {
                        xgfx::Operator::Over
                    } else {
                        xgfx::Operator::MultiplyThenOver(palette.resolve(&attrs.foreground).into())
                    };
                    self.buffer_image.draw_image(
                        x + x_offset as isize + bearing_x,
                        y + self.descender - (y_offset as isize + bearing_y),
                        &image,
                        operator,
                    );
                }

                x += x_advance as isize;
                y += y_advance as isize;
            }
        }

        Ok(())
    }
}

fn run() -> Result<(), Error> {
    let (conn, screen_num) = xcb::Connection::connect(None)?;
    println!("Connected screen {}", screen_num);

    let mut terminal = term::Terminal::new(24, 80, 3000);
    let message = "x_advance != \x1b[38;2;1;0;125;145;mfoo->bar(); ❤ 😍🤢\n\x1b[91;mw00t\n\x1b[37;104;m bleet\x1b[0;m.";
    terminal.advance_bytes(message);

    let mut window = TerminalWindow::new(&conn, screen_num, 1024, 300, terminal)?;
    window.show();

    conn.flush();

    loop {
        // If we need to re-render the display, try to defer that until after we've
        // consumed the input queue
        let event = if window.need_paint {
            match conn.poll_for_queued_event() {
                None => {
                    window.paint()?;
                    conn.flush();
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

fn ptyrun() -> Result<(), Error> {
    use std::process::Command;
    use std::io::Read;

    let mut cmd = Command::new("ls");

    let (mut master, slave) = pty::openpty(24, 80)?;

    let child = slave.spawn_command(cmd)?;
    eprintln!("spawned: {:?}", child);

    loop {
        let mut buf = [0;256];

        match master.read(&mut buf) {
            Ok(size) => println!("[ls] {}", std::str::from_utf8(&buf[0..size]).unwrap()),
            Err(err) => {
                eprintln!("[ls:err] {:?}", err);
                break
            }
        }
    }

    Ok(())
}

fn main() {
    ptyrun().unwrap();
//    run().unwrap();
}
