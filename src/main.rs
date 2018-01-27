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
extern crate mio;
#[macro_use]
pub mod log;

use failure::Error;

extern crate xcb;
extern crate xcb_util;

use mio::{Events, Poll, PollOpt, Ready, Token};
use mio::unix::EventedFd;
use std::io::Read;
use std::mem;
use std::os::unix::io::AsRawFd;
use std::process::{Child, Command};
use std::slice;
use std::time::Duration;

mod xgfx;
use xgfx::Drawable;
mod font;
use font::{Font, FontPattern, ftwrap};

mod term;
mod pty;
use pty::MasterPty;

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
    pty: MasterPty,
    process: Child,
}

impl<'a> TerminalWindow<'a> {
    fn new(
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

            let rows = height / self.cell_height as u16;
            let cols = width / self.cell_width as u16;
            self.pty.resize(rows, cols, width, height);
            self.terminal.resize(rows as usize, cols as usize);

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

    fn handle_pty_readable_event(&mut self) {
        const kBufSize: usize = 8192;
        let mut buf = [0; kBufSize];

        loop {
            match self.pty.read(&mut buf) {
                Ok(size) => {
                    self.terminal.advance_bytes(&buf[0..size]);
                    self.need_paint = true;
                    if size < kBufSize {
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
}

fn dispatch_gui(
    event: xcb::GenericEvent,
    window: &mut TerminalWindow,
    atom_delete: xcb::Atom,
) -> Result<(), Error> {
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
            println!("Key '{}' pressed", key_press.detail());
        }
        xcb::CLIENT_MESSAGE => {
            let msg: &xcb::ClientMessageEvent = unsafe { xcb::cast_event(&event) };
            println!("CLIENT_MESSAGE {:?}", msg.data().data32());
            if msg.data().data32()[0] == atom_delete {
                // TODO: cleaner exit handling
                bail!("window close requested!");
            }
        }
        _ => {}
    }
    Ok(())
}

fn run() -> Result<(), Error> {
    let poll = Poll::new()?;
    let (conn, screen_num) = xcb::Connection::connect(None)?;

    // First step is to figure out the font metrics so that we know how
    // big things are going to be.

    let mut pattern = FontPattern::parse("Operator Mono SSm Lig:size=12")?;
    pattern.add_double("dpi", 96.0)?;
    let mut font = Font::new(pattern)?;
    // we always load the cell_height for font 0,
    // regardless of which font we are shaping here,
    // so that we can scale glyphs appropriately
    let (cell_height, cell_width, _) = font.get_metrics()?;

    let initial_cols = 80u16;
    let initial_rows = 24u16;
    let initial_pixel_width = initial_cols * cell_width.ceil() as u16;
    let initial_pixel_height = initial_rows * cell_height.ceil() as u16;

    let (master, slave) = pty::openpty(
        initial_rows,
        initial_cols,
        initial_pixel_width,
        initial_pixel_height,
    )?;

    let mut cmd = Command::new("top");
    //    cmd.arg("-l");
    let child = slave.spawn_command(cmd)?;
    eprintln!("spawned: {:?}", child);

    // Ask mio to watch the pty for input from the child process
    poll.register(
        &master,
        Token(0),
        Ready::readable(),
        PollOpt::edge(),
    )?;
    // Ask mio to monitor the X connection fd
    poll.register(
        &EventedFd(&conn.as_raw_fd()),
        Token(1),
        Ready::readable(),
        PollOpt::edge(),
    )?;

    let terminal = term::Terminal::new(initial_rows as usize, initial_cols as usize, 3000);
    //    let message = "x_advance != \x1b[38;2;1;0;125;145;mfoo->bar(); ❤ 😍🤢\n\x1b[91;mw00t\n\x1b[37;104;m bleet\x1b[0;m.";
    //    terminal.advance_bytes(message);

    let mut window = TerminalWindow::new(
        &conn,
        screen_num,
        initial_pixel_width,
        initial_pixel_height,
        terminal,
        master,
        child,
        font,
    )?;
    let atom_protocols = xcb::intern_atom(&conn, false, "WM_PROTOCOLS")
        .get_reply()?
        .atom();
    let atom_delete = xcb::intern_atom(&conn, false, "WM_DELETE_WINDOW")
        .get_reply()?
        .atom();
    xcb::change_property(
        &conn,
        xcb::PROP_MODE_REPLACE as u8,
        window.window.as_drawable(),
        atom_protocols,
        4,
        32,
        &[atom_delete],
    );

    window.show();

    let mut events = Events::with_capacity(8);
    conn.flush();

    loop {
        if poll.poll(&mut events, Some(Duration::new(0, 0)))? == 0 {
            // No immediately ready events.  Before we go to sleep,
            // make sure we've flushed out any pending X work.
            if window.need_paint {
                window.paint()?;
            }
            conn.flush();

            poll.poll(&mut events, None)?;
        }

        /*
        match child.try_wait() {
            Ok(Some(status)) => {
                println!("child exited: {}", status);
                break;
            }
            Ok(None) => println!("child still running"),
            Err(e) => {
                println!("failed to wait for child: {}", e);
                break;
            }
        }
*/

        for event in &events {
            if event.token() == Token(0) && event.readiness().is_readable() {
                window.handle_pty_readable_event();
            }
            if event.token() == Token(1) && event.readiness().is_readable() {
                // Each time the XCB Connection FD shows as readable, we perform
                // a single poll against the connection and then eagerly consume
                // all of the queued events that came along as part of that batch.
                // This is important because we can't assume that one readiness
                // event from the kerenl maps to a single XCB event.  We need to be
                // sure that all buffered/queued events are consumed before we
                // allow the mio poll() routine to put us to sleep, otherwise we
                // will effectively hang without updating all the state.
                match conn.poll_for_event() {
                    Some(event) => {
                        dispatch_gui(event, &mut window, atom_delete)?;
                        // Since we read one event from the connection, we must
                        // now eagerly consume the rest of the queued events.
                        loop {
                            match conn.poll_for_queued_event() {
                                Some(event) => dispatch_gui(event, &mut window, atom_delete)?,
                                None => break,
                            }
                        }
                    }
                    None => {}
                }

                // If we got disconnected from the display server, we cannot continue
                conn.has_error()?;
            }
        }
    }
}

fn main() {
    run().unwrap();
}
