use config::TextStyle;
use failure::{self, Error};
use font::{FontConfiguration, GlyphInfo, ftwrap};
use pty::MasterPty;
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::mem;
use std::process::Child;
use std::rc::Rc;
use std::slice;
use term::{self, KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind, TerminalHost};
use xcb;
use xcb_util;
use xgfx::{self, BitmapImage, Connection, Drawable};
use xkeysyms;

/// BufferImage is used to hold the bitmap of our rendered screen.
/// If SHM is available we store it there and save the overhead of
/// sending the bitmap to the server each time something is rendered.
/// Otherwise, we will send up portions of the bitmap each time something
/// on the screen changes.
enum BufferImage<'a> {
    Shared(xgfx::ShmImage<'a>),
    Image(xgfx::Image),
}

impl<'a> BufferImage<'a> {
    fn new(conn: &Connection, drawable: xcb::Drawable, width: usize, height: usize) -> BufferImage {
        match xgfx::ShmImage::new(conn, drawable, width, height) {
            Ok(shm) => BufferImage::Shared(shm),
            Err(err) => {
                debug!("falling back to local image because SHM says: {:?}", err);
                BufferImage::Image(xgfx::Image::new(width, height))
            }
        }
    }
}

/// Implement BitmapImage that delegates to the underlying image
impl<'a> BitmapImage for BufferImage<'a> {
    unsafe fn pixel_data(&self) -> *const u8 {
        match self {
            &BufferImage::Shared(ref shm) => shm.pixel_data(),
            &BufferImage::Image(ref im) => im.pixel_data(),
        }
    }

    unsafe fn pixel_data_mut(&mut self) -> *mut u8 {
        match self {
            &mut BufferImage::Shared(ref mut shm) => shm.pixel_data_mut(),
            &mut BufferImage::Image(ref mut im) => im.pixel_data_mut(),
        }
    }

    fn image_dimensions(&self) -> (usize, usize) {
        match self {
            &BufferImage::Shared(ref shm) => shm.image_dimensions(),
            &BufferImage::Image(ref im) => im.image_dimensions(),
        }
    }
}

/// Holds the information we need to implement TerminalHost
struct Host<'a> {
    window: xgfx::Window<'a>,
    pty: MasterPty,
    timestamp: xcb::xproto::Timestamp,
    clipboard: Option<String>,
}

pub struct TerminalWindow<'a> {
    host: Host<'a>,
    conn: &'a Connection,
    width: u16,
    height: u16,
    fonts: FontConfiguration,
    cell_height: f64,
    cell_width: f64,
    descender: isize,
    window_context: xgfx::Context<'a>,
    buffer_image: BufferImage<'a>,
    terminal: term::Terminal,
    process: Child,
    glyph_cache: RefCell<HashMap<GlyphKey, Rc<CachedGlyph>>>,
    palette: term::color::ColorPalette,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct GlyphKey {
    font_idx: usize,
    glyph_pos: u32,
    style: TextStyle,
}

/// Caches a rendered glyph.
/// The image data may be None for whitespace glyphs.
struct CachedGlyph {
    image: Option<xgfx::Image>,
    scale: f64,
    has_color: bool,
    x_advance: isize,
    y_advance: isize,
    x_offset: isize,
    y_offset: isize,
    bearing_x: isize,
    bearing_y: isize,
}

impl<'a> term::TerminalHost for Host<'a> {
    fn writer(&mut self) -> &mut Write {
        &mut self.pty
    }

    // Check out https://tronche.com/gui/x/icccm/sec-2.html for some deep and complex
    // background on what's happening in here.
    fn get_clipboard(&mut self) -> Result<String, Error> {
        // If we own the clipboard, just return the text now
        if let Some(ref text) = self.clipboard {
            return Ok(text.clone());
        }

        let conn = self.window.get_conn();

        xcb::convert_selection(
            conn.conn(),
            self.window.as_drawable(),
            xcb::ATOM_PRIMARY,
            conn.atom_utf8_string,
            conn.atom_xsel_data,
            self.timestamp,
        );
        conn.flush();

        loop {
            let event = conn.wait_for_event().ok_or_else(
                || failure::err_msg("X connection EOF"),
            )?;
            match event.response_type() & 0x7f {
                xcb::SELECTION_NOTIFY => {
                    let selection: &xcb::SelectionNotifyEvent = unsafe { xcb::cast_event(&event) };

                    if selection.selection() == xcb::ATOM_PRIMARY &&
                        selection.property() != xcb::NONE
                    {
                        let prop = xcb_util::icccm::get_text_property(
                            conn,
                            selection.requestor(),
                            selection.property(),
                        ).get_reply()?;
                        return Ok(prop.name().into());
                    }
                }
                _ => {
                    eprintln!(
                        "whoops: got XCB event type {} while waiting for selection",
                        event.response_type() & 0x7f
                    );
                    // Rather than block forever, give up and yield an empty string
                    // for pasting purposes.  We lost an event.  This sucks.
                    // Will likely need to rethink how we handle passing the clipboard
                    // data down to the terminal.
                    return Ok("".into());
                }
            }
        }
    }

    fn set_clipboard(&mut self, clip: Option<String>) -> Result<(), Error> {
        self.clipboard = clip;
        let conn = self.window.get_conn();

        xcb::set_selection_owner(
            conn.conn(),
            if self.clipboard.is_some() {
                self.window.as_drawable()
            } else {
                xcb::NONE
            },
            xcb::ATOM_PRIMARY,
            self.timestamp,
        );

        // TODO: icccm says that we should check that we got ownership and
        // amend our UI accordingly

        Ok(())
    }
}

impl<'a> TerminalWindow<'a> {
    pub fn new(
        conn: &Connection,
        width: u16,
        height: u16,
        terminal: term::Terminal,
        pty: MasterPty,
        process: Child,
        fonts: FontConfiguration,
    ) -> Result<TerminalWindow, Error> {
        let (cell_height, cell_width, descender) = {
            // Urgh, this is a bit repeaty, but we need to satisfy the borrow checker
            let font = fonts.default_font()?;
            let tuple = font.borrow_mut().get_metrics()?;
            tuple
        };

        let window = xgfx::Window::new(&conn, width, height)?;
        window.set_title("wezterm");
        let window_context = xgfx::Context::new(conn, &window);

        let buffer_image =
            BufferImage::new(conn, window.as_drawable(), width as usize, height as usize);

        let descender = if descender.is_positive() {
            ((descender as f64) / 64.0).ceil() as isize
        } else {
            ((descender as f64) / 64.0).floor() as isize
        };

        Ok(TerminalWindow {
            host: Host {
                window,
                pty,
                timestamp: 0,
                clipboard: None,
            },
            window_context,
            buffer_image,
            conn,
            width,
            height,
            fonts,
            cell_height,
            cell_width,
            descender,
            terminal,
            process,
            glyph_cache: RefCell::new(HashMap::new()),
            palette: term::color::ColorPalette::default(),
        })
    }

    pub fn show(&self) {
        self.host.window.show();
    }

    pub fn resize_surfaces(&mut self, width: u16, height: u16) -> Result<bool, Error> {
        if width != self.width || height != self.height {
            debug!("resize {},{}", width, height);

            let mut buffer = BufferImage::new(
                self.conn,
                self.host.window.as_drawable(),
                width as usize,
                height as usize,
            );
            buffer.draw_image(0, 0, &self.buffer_image, xgfx::Operator::Source);
            self.buffer_image = buffer;

            self.width = width;
            self.height = height;

            let rows = (height as f64 / self.cell_height).floor() as u16;
            let cols = (width as f64 / self.cell_width).floor() as u16;
            self.host.pty.resize(rows, cols, width, height)?;
            self.terminal.resize(rows as usize, cols as usize);

            // If we have partial rows or columns to the bottom or right,
            // clear those out as they may contains artifacts from prior to
            // the resize.
            let background_color = self.palette.resolve(
                &term::color::ColorAttribute::Background,
            );
            self.buffer_image.clear_rect(
                cols as isize * self.cell_width as isize,
                0,
                width as usize - (cols as usize * self.cell_width as usize),
                self.height as usize,
                background_color.into(),
            );
            self.buffer_image.clear_rect(
                0,
                rows as isize * self.cell_height as isize,
                width as usize,
                height as usize - (rows as usize * self.cell_height as usize),
                background_color.into(),
            );

            Ok(true)
        } else {
            debug!("ignoring extra resize");
            Ok(false)
        }
    }

    pub fn expose(&mut self, x: u16, y: u16, width: u16, height: u16) -> Result<(), Error> {
        debug!("expose {},{}, {},{}", x, y, width, height);

        match &self.buffer_image {
            &BufferImage::Shared(ref shm) => {
                self.window_context.copy_area(
                    shm,
                    x as i16,
                    y as i16,
                    &self.host.window,
                    x as i16,
                    y as i16,
                    width,
                    height,
                );
            }
            &BufferImage::Image(ref buffer) => {
                if x == 0 && y == 0 && width == self.width && height == self.height {
                    self.window_context.put_image(0, 0, buffer);
                } else {
                    let mut im = xgfx::Image::new(width as usize, height as usize);
                    im.draw_image_subset(
                        0,
                        0,
                        x as usize,
                        y as usize,
                        width as usize,
                        height as usize,
                        buffer,
                        xgfx::Operator::Source,
                    );
                    self.window_context.put_image(x as i16, y as i16, &im);
                }
            }
        }
        self.conn.flush();

        Ok(())
    }

    /// Resolve a glyph from the cache, rendering the glyph on-demand if
    /// the cache doesn't already hold the desired glyph.
    fn cached_glyph(&self, info: &GlyphInfo, style: &TextStyle) -> Result<Rc<CachedGlyph>, Error> {
        let key = GlyphKey {
            font_idx: info.font_idx,
            glyph_pos: info.glyph_pos,
            style: style.clone(),
        };

        let mut cache = self.glyph_cache.borrow_mut();

        if let Some(entry) = cache.get(&key) {
            return Ok(Rc::clone(entry));
        }

        let glyph = self.load_glyph(info, style)?;
        cache.insert(key, Rc::clone(&glyph));
        Ok(glyph)
    }

    /// Perform the load and render of a glyph
    fn load_glyph(&self, info: &GlyphInfo, style: &TextStyle) -> Result<Rc<CachedGlyph>, Error> {
        let (has_color, ft_glyph, cell_width, cell_height) = {
            let font = self.fonts.cached_font(style)?;
            let mut font = font.borrow_mut();
            let (height, width, _) = font.get_metrics()?;
            let has_color = font.has_color(info.font_idx)?;
            // This clone is conceptually unsafe, but ok in practice as we are
            // single threaded and don't load any other glyphs in the body of
            // this load_glyph() function.
            let ft_glyph = font.load_glyph(info.font_idx, info.glyph_pos)?.clone();
            (has_color, ft_glyph, width, height)
        };

        let scale = if (info.x_advance / info.num_cells as f64).floor() > cell_width {
            info.num_cells as f64 * (cell_width / info.x_advance)
        } else if ft_glyph.bitmap.rows as f64 > cell_height {
            cell_height / ft_glyph.bitmap.rows as f64
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

        let glyph = if ft_glyph.bitmap.width == 0 || ft_glyph.bitmap.rows == 0 {
            // a whitespace glyph
            CachedGlyph {
                image: None,
                scale,
                has_color,
                x_advance: x_advance as isize,
                y_advance: y_advance as isize,
                x_offset: x_offset as isize,
                y_offset: y_offset as isize,
                bearing_x: 0,
                bearing_y: 0,
            }
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

            let image = if scale != 1.0 {
                image.scale_by(scale)
            } else {
                image
            };

            #[cfg(debug_assertions)]
            {
                if info.text == "X" {
                    println!(
                        "X: x_advance={} x_offset={} bearing_x={} image={:?} info={:?} glyph={:?}",
                        x_advance,
                        x_offset,
                        bearing_x,
                        image.image_dimensions(),
                        info,
                        ft_glyph
                    );
                }
            }

            CachedGlyph {
                image: Some(image),
                scale,
                has_color,
                x_advance: x_advance as isize,
                y_advance: y_advance as isize,
                x_offset: x_offset as isize,
                y_offset: y_offset as isize,
                bearing_x,
                bearing_y,
            }
        };

        Ok(Rc::new(glyph))
    }

    /// A little helper for shaping text.
    /// This is needed to dance around interior mutability concerns,
    /// as the font caches things.
    /// TODO: consider pushing this down into the Font impl itself.
    fn shape_text(&self, s: &str, style: &TextStyle) -> Result<Vec<GlyphInfo>, Error> {
        let font = self.fonts.cached_font(style)?;
        let mut font = font.borrow_mut();
        font.shape(0, s)
    }

    pub fn paint(&mut self) -> Result<(), Error> {
        let background_color = self.palette.resolve(
            &term::color::ColorAttribute::Background,
        );

        let cell_height = self.cell_height.ceil() as usize;
        let cell_width = self.cell_width.ceil() as usize;

        let cursor = self.terminal.cursor_pos();
        {
            let dirty_lines = self.terminal.get_dirty_lines();

            for (line_idx, line, selrange) in dirty_lines {

                let mut x = 0 as isize;
                let y = (line_idx * cell_height) as isize;

                // Clear this dirty row
                self.buffer_image.clear_rect(
                    0,
                    y,
                    self.width as usize,
                    cell_height,
                    background_color.into(),
                );

                // Break the line into clusters of cells with the same attributes
                let cell_clusters = line.cluster();
                for cluster in cell_clusters {
                    let attrs = &cluster.attrs;
                    let style = self.fonts.match_style(attrs);
                    let metric_width = {
                        let font = self.fonts.cached_font(style)?;
                        let (_, width, _) = font.borrow_mut().get_metrics()?;
                        width as usize
                    };

                    let (fg_color, bg_color) = {
                        let mut fg_color = &attrs.foreground;
                        let mut bg_color = &attrs.background;

                        if attrs.reverse() {
                            mem::swap(&mut fg_color, &mut bg_color);
                        }

                        (fg_color, bg_color)
                    };

                    let bg_color = self.palette.resolve(bg_color);

                    // Shape the printable text from this cluster
                    let glyph_info = self.shape_text(&cluster.text, &style)?;
                    for info in glyph_info.iter() {
                        let cell_idx = cluster.byte_to_cell_idx[info.cluster as usize];
                        let cell = &line.cells[cell_idx];
                        let cell_print_width = cell.width();

                        // Render the cell background color
                        self.buffer_image.clear_rect(
                            x,
                            y,
                            cell_print_width * metric_width,
                            cell_height,
                            bg_color.into(),
                        );


                        // Render selection background
                        for cur_x in cell_idx..cell_idx + info.num_cells as usize {
                            if term::in_range(cur_x, &selrange) {
                                self.buffer_image.clear_rect(
                                    (cur_x * metric_width) as isize,
                                    y,
                                    cell_width * line.cells[cur_x].width(),
                                    cell_height,
                                    self.palette.cursor().into(),
                                );
                            }
                        }

                        // Render the cursor, if it overlaps with the current cluster
                        if line_idx as i64 == cursor.y {
                            for cur_x in cell_idx..cell_idx + info.num_cells as usize {
                                if cursor.x == cur_x {
                                    // The cursor fits in this cell, so render the cursor bg
                                    self.buffer_image.clear_rect(
                                        (cur_x * metric_width) as isize,
                                        y,
                                        cell_width * line.cells[cur_x].width(),
                                        cell_height,
                                        self.palette.cursor().into(),
                                    );
                                }
                            }
                        }

                        let glyph = self.cached_glyph(info, &style)?;
                        // glyph.image.is_none() for whitespace glyphs
                        if let &Some(ref image) = &glyph.image {
                            if false && line_idx == cursor.y as usize {
                                debug!(
                                    "x,y: {},{} desc={} bearing:{},{} off={},{} adv={},{} scale={}",
                                    x,
                                    y,
                                    self.descender,
                                    glyph.bearing_x,
                                    glyph.bearing_y,
                                    glyph.x_offset,
                                    glyph.y_offset,
                                    glyph.x_advance,
                                    glyph.y_advance,
                                    glyph.scale
                                );
                            }


                            let (glyph_width, glyph_height) = image.image_dimensions();

                            // This is a little bit tricky.
                            // We may have a double-wide glyph based on double-wide contents
                            // of the cell, or we may have a double-wide glyph based on
                            // the result of shaping a contextual ligature.  The computed
                            // cell_print_width tells us about the former.  The shaping
                            // data in info.num_cells includes both cases.  In order not
                            // to skip out on rendering the cursor we need to slice the
                            // glyph up into cell strips and render them.
                            for slice_x in 0..info.num_cells as usize {
                                let is_cursor = cursor.x == slice_x + cell_idx &&
                                    line_idx as i64 == cursor.y;

                                let glyph_color = if is_cursor {
                                    // overlaps with cursor, so adjust colors.
                                    // TODO: could make cursor fg color an option.
                                    &attrs.background
                                } else {
                                    fg_color
                                };
                                let glyph_color = match glyph_color {
                                    &term::color::ColorAttribute::PaletteIndex(idx) if idx < 8 => {
                                        // For compatibility purposes, switch to a brighter version
                                        // of one of the standard ANSI colors when Bold is enabled.
                                        // This lifts black to dark grey.
                                        let idx = if attrs.intensity() == term::Intensity::Bold {
                                            idx + 8
                                        } else {
                                            idx
                                        };
                                        self.palette.resolve(
                                            &term::color::ColorAttribute::PaletteIndex(idx),
                                        )
                                    }
                                    _ => self.palette.resolve(glyph_color),
                                };
                                let operator = if glyph.has_color {
                                    xgfx::Operator::Over
                                } else {
                                    xgfx::Operator::MultiplyThenOver(glyph_color.into())
                                };

                                let slice_offset = slice_x * metric_width;

                                // How much of the glyph to render in this slice.  If we're
                                // the last slice in the sequence then we don't clamp to the
                                // cell metrics so that ligatures can bleed from one of the
                                // slice/cells into the next and look good.
                                let slice_width = if slice_x == info.num_cells as usize - 1 {
                                    glyph_width - slice_offset
                                } else {
                                    (glyph_width - slice_offset).min(metric_width)
                                };

                                self.buffer_image.draw_image_subset(
                                    slice_offset as isize + x + glyph.x_offset as isize +
                                        glyph.bearing_x,
                                    y + cell_height as isize + self.descender -
                                        (glyph.y_offset as isize + glyph.bearing_y),
                                    slice_offset,
                                    0,
                                    slice_width,
                                    glyph_height.min(cell_height),
                                    image,
                                    operator,
                                );

                                if is_cursor {
                                    // Render a block outline style of cursor.
                                    // TODO: make this respect user configuration
                                    self.buffer_image.draw_rect(
                                        (slice_offset + (cell_idx * metric_width)) as
                                            isize,
                                        y,
                                        // take care to use the print width here otherwise
                                        // the rectangle will incorrectly bisect the glyph
                                        (cell_print_width * metric_width),
                                        cell_height,
                                        self.palette.cursor().into(),
                                        xgfx::Operator::Over,
                                    );
                                }
                            }
                        }

                        x += glyph.x_advance;
                    }
                }

                // If we have SHM available, we can send up just this changed line
                match &self.buffer_image {
                    &BufferImage::Shared(ref shm) => {
                        self.window_context.copy_area(
                            shm,
                            0,
                            y as i16,
                            &self.host.window,
                            0,
                            y as i16,
                            self.width,
                            cell_height as u16,
                        );
                    }
                    &BufferImage::Image(_) => {
                        // Will handle this at the end
                    }
                }

            }
        }

        match &self.buffer_image {
            &BufferImage::Shared(_) => {
                // We handled this above
            }
            &BufferImage::Image(ref buffer) => {
                // With no SHM available, we have to push the whole screen buffer
                // here, regardless of which lines are dirty.
                self.window_context.put_image(0, 0, buffer);
            }
        }

        self.terminal.clean_dirty_lines();

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
            match self.host.pty.read(&mut buf) {
                Ok(size) => {
                    for answer in self.terminal.advance_bytes(&buf[0..size]) {
                        match answer {
                            term::AnswerBack::WriteToPty(response) => {
                                self.host.pty.write(&response).ok(); // discard error
                            }
                            term::AnswerBack::TitleChanged(title) => {
                                self.host.window.set_title(&title);
                            }
                        }
                    }
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
        self.terminal.has_dirty_lines()
    }

    fn decode_key(&self, event: &xcb::KeyPressEvent) -> (KeyCode, KeyModifiers) {
        let mods = xkeysyms::modifiers(event);
        let sym = self.conn.lookup_keysym(
            event,
            mods.contains(KeyModifiers::SHIFT),
        );
        (xkeysyms::xcb_keysym_to_keycode(sym), mods)
    }

    fn clear_selection(&mut self) -> Result<(), Error> {
        self.host.set_clipboard(None)?;
        self.terminal.clear_selection();
        Ok(())
    }

    fn mouse_event(&mut self, event: MouseEvent) -> Result<(), Error> {
        self.terminal.mouse_event(event, &mut self.host)?;
        Ok(())
    }

    pub fn dispatch_event(&mut self, event: xcb::GenericEvent) -> Result<(), Error> {
        let r = event.response_type() & 0x7f;
        match r {
            xcb::EXPOSE => {
                let expose: &xcb::ExposeEvent = unsafe { xcb::cast_event(&event) };
                self.expose(
                    expose.x(),
                    expose.y(),
                    expose.width(),
                    expose.height(),
                )?;
            }
            xcb::CONFIGURE_NOTIFY => {
                let cfg: &xcb::ConfigureNotifyEvent = unsafe { xcb::cast_event(&event) };
                self.resize_surfaces(cfg.width(), cfg.height())?;
            }
            xcb::KEY_PRESS => {
                let key_press: &xcb::KeyPressEvent = unsafe { xcb::cast_event(&event) };
                self.host.timestamp = key_press.time();
                let (code, mods) = self.decode_key(key_press);
                self.terminal.key_down(code, mods, &mut self.host)?;
            }
            xcb::KEY_RELEASE => {
                let key_press: &xcb::KeyPressEvent = unsafe { xcb::cast_event(&event) };
                self.host.timestamp = key_press.time();
                let (code, mods) = self.decode_key(key_press);
                self.terminal.key_up(code, mods, &mut self.host)?;
            }
            xcb::MOTION_NOTIFY => {
                let motion: &xcb::MotionNotifyEvent = unsafe { xcb::cast_event(&event) };

                let event = MouseEvent {
                    kind: MouseEventKind::Move,
                    button: MouseButton::None,
                    x: (motion.event_x() as f64 / self.cell_width).floor() as usize,
                    y: (motion.event_y() as f64 / self.cell_height).floor() as i64,
                    modifiers: xkeysyms::modifiers_from_state(motion.state()),
                };
                self.mouse_event(event)?;
            }
            xcb::BUTTON_PRESS |
            xcb::BUTTON_RELEASE => {
                let button_press: &xcb::ButtonPressEvent = unsafe { xcb::cast_event(&event) };
                self.host.timestamp = button_press.time();

                let event = MouseEvent {
                    kind: match r {
                        xcb::BUTTON_PRESS => MouseEventKind::Press,
                        xcb::BUTTON_RELEASE => MouseEventKind::Release,
                        _ => unreachable!("button event mismatch"),
                    },
                    x: (button_press.event_x() as f64 / self.cell_width).floor() as usize,
                    y: (button_press.event_y() as f64 / self.cell_height).floor() as i64,
                    button: match button_press.detail() {
                        1 => MouseButton::Left,
                        2 => MouseButton::Middle,
                        3 => MouseButton::Right,
                        4 => MouseButton::WheelUp,
                        5 => MouseButton::WheelDown,
                        _ => {
                            eprintln!("button {} is not implemented", button_press.detail());
                            return Ok(());
                        }
                    },
                    modifiers: xkeysyms::modifiers_from_state(button_press.state()),
                };

                self.mouse_event(event)?;
            }
            xcb::CLIENT_MESSAGE => {
                let msg: &xcb::ClientMessageEvent = unsafe { xcb::cast_event(&event) };
                println!("CLIENT_MESSAGE {:?}", msg.data().data32());
                if msg.data().data32()[0] == self.conn.atom_delete() {
                    // TODO: cleaner exit handling
                    bail!("window close requested!");
                }
            }
            xcb::SELECTION_CLEAR => {
                // Someone else now owns the selection
                self.clear_selection()?;
            }
            xcb::SELECTION_REQUEST => {
                // Someone is asking for our selected text

                let request: &xcb::SelectionRequestEvent = unsafe { xcb::cast_event(&event) };
                debug!(
                    "SEL: time={} owner={} requestor={} selection={} target={} property={}",
                    request.time(),
                    request.owner(),
                    request.requestor(),
                    request.selection(),
                    request.target(),
                    request.property()
                );
                debug!(
                    "XSEL={}, UTF8={} PRIMARY={}",
                    self.conn.atom_xsel_data,
                    self.conn.atom_utf8_string,
                    xcb::ATOM_PRIMARY,
                );


                // I'd like to use `match` here, but the atom values are not
                // known at compile time so we have to `if` like a caveman :-p
                let selprop = if request.target() == self.conn.atom_targets {
                    // They want to know which targets we support
                    let atoms: [u32; 1] = [self.conn.atom_utf8_string];
                    xcb::xproto::change_property(
                        self.conn.conn(),
                        xcb::xproto::PROP_MODE_REPLACE as u8,
                        request.requestor(),
                        request.property(),
                        xcb::xproto::ATOM_ATOM,
                        32, /* 32-bit atom value */
                        &atoms,
                    );

                    // let the requestor know that we set their property
                    request.property()

                } else if request.target() == self.conn.atom_utf8_string ||
                           request.target() == xcb::xproto::ATOM_STRING
                {
                    // We'll accept requests for UTF-8 or STRING data.
                    // We don't and won't do any conversion from UTF-8 to
                    // whatever STRING represents; let's just assume that
                    // the other end is going to handle it correctly.
                    if let &Some(ref text) = &self.host.clipboard {
                        xcb::xproto::change_property(
                            self.conn.conn(),
                            xcb::xproto::PROP_MODE_REPLACE as u8,
                            request.requestor(),
                            request.property(),
                            request.target(),
                            8, /* 8-bit string data */
                            text.as_bytes(),
                        );
                        // let the requestor know that we set their property
                        request.property()
                    } else {
                        // We have no clipboard so there is nothing to report
                        xcb::NONE
                    }
                } else {
                    // We didn't support their request, so there is nothing
                    // we can report back to them.
                    xcb::NONE
                };

                xcb::xproto::send_event(
                    self.conn.conn(),
                    true,
                    request.requestor(),
                    0,
                    &xcb::xproto::SelectionNotifyEvent::new(
                        request.time(),
                        request.requestor(),
                        request.selection(),
                        request.target(),
                        selprop, // the disposition from the operation above
                    ),
                );
            }
            _ => {}
        }
        Ok(())
    }
}
