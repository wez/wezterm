
use failure::Error;
use font::{Font, ftwrap};
use pty::MasterPty;
use std::io::Read;
use std::mem;
use std::process::Child;
use std::slice;
use term;
use xcb;
use xgfx::{self, Drawable};

pub struct TerminalWindow<'a> {
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
    pty: MasterPty,
    process: Child,
}

impl<'a> TerminalWindow<'a> {
    pub fn new(
        conn: &xcb::Connection,
        screen_num: i32,
        width: u16,
        height: u16,
        terminal: term::Terminal,
        pty: MasterPty,
        process: Child,
        mut font: Font,
    ) -> Result<TerminalWindow, Error> {
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
            pty,
            process,
        })
    }

    pub fn window_id(&self) -> u32 {
        self.window.as_drawable()
    }

    pub fn show(&self) {
        self.window.show();
    }

    pub fn resize_surfaces(&mut self, width: u16, height: u16) -> Result<bool, Error> {
        if width != self.width || height != self.height {
            debug!("resize {},{}", width, height);
            let mut buffer = xgfx::Image::new(width as usize, height as usize);
            buffer.draw_image(0, 0, &self.buffer_image, xgfx::Operator::Source);
            self.buffer_image = buffer;
            self.width = width;
            self.height = height;

            let rows = height / self.cell_height as u16;
            let cols = width / self.cell_width as u16;
            self.pty.resize(rows, cols, width, height)?;
            self.terminal.resize(rows as usize, cols as usize);

            self.need_paint = true;
            Ok(true)
        } else {
            debug!("ignoring extra resize");
            Ok(false)
        }
    }

    pub fn expose(&mut self, x: u16, y: u16, width: u16, height: u16) -> Result<(), Error> {
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

    pub fn paint(&mut self) -> Result<(), Error> {
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
                if cell_idx > phys_cols {
                    break;
                }
                let has_color = self.font.has_color(info.font_idx)?;
                let ft_glyph = self.font.load_glyph(info.font_idx, info.glyph_pos)?;

                let attrs = &line.cells[cell_idx].attrs;

                let (fg_color, bg_color) = if attrs.reverse() {
                    (
                        palette.resolve(&attrs.background),
                        palette.resolve(&attrs.foreground),
                    )
                } else {
                    (
                        palette.resolve(&attrs.foreground),
                        palette.resolve(&attrs.background),
                    )
                };

                // Render the cell background color
                self.buffer_image.clear_rect(
                    x,
                    y - cell_height as isize,
                    info.num_cells as usize * self.cell_width as usize,
                    cell_height,
                    bg_color.into(),
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
                        xgfx::Operator::MultiplyThenOver(fg_color.into())
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

        // FIXME: we have to push the render to the server in case it
        // was the result of output from the process on the pty.  It would
        // be nice to make this paint function only re-render the changed
        // portions and send only those to the X server here.
        self.window_context.put_image(0, 0, &self.buffer_image);

        Ok(())
    }

    pub fn test_for_child_exit(&mut self) -> Result<(), Error> {
        match self.process.try_wait() {
            Ok(Some(status)) => {
                bail!("child exited: {}", status);
            }
            Ok(None) => {
                println!("child still running");
                Ok(())
            }
            Err(e) => {
                bail!("failed to wait for child: {}", e);
            }
        }
    }

    pub fn handle_pty_readable_event(&mut self) {
        const BUFSIZE: usize = 8192;
        let mut buf = [0; BUFSIZE];

        loop {
            match self.pty.read(&mut buf) {
                Ok(size) => {
                    self.terminal.advance_bytes(&buf[0..size]);
                    self.need_paint = true;
                    if size < BUFSIZE {
                        // If we had a short read then there is no more
                        // data to read right now; we'll get called again
                        // when mio says that we're ready
                        break;
                    }
                }
                Err(err) => {
                    eprintln!("error reading from pty: {:?}", err);
                    break;
                }
            }
        }
    }

    pub fn need_paint(&self) -> bool {
        self.need_paint
    }
}
