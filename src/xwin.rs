use failure::Error;
use font::{Font, GlyphInfo, ftwrap};
use pty::MasterPty;
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::mem;
use std::process::Child;
use std::rc::Rc;
use std::slice;
use term::{self, KeyCode, KeyModifiers};
use xcb;
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

pub struct TerminalWindow<'a> {
    window: xgfx::Window<'a>,
    conn: &'a Connection,
    width: u16,
    height: u16,
    font: RefCell<Font>,
    cell_height: f64,
    cell_width: f64,
    descender: isize,
    window_context: xgfx::Context<'a>,
    buffer_image: BufferImage<'a>,
    terminal: term::Terminal,
    pty: MasterPty,
    process: Child,
    glyph_cache: RefCell<HashMap<GlyphKey, Rc<CachedGlyph>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct GlyphKey {
    font_idx: usize,
    glyph_pos: u32,
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

impl<'a> TerminalWindow<'a> {
    pub fn new(
        conn: &Connection,
        width: u16,
        height: u16,
        terminal: term::Terminal,
        pty: MasterPty,
        process: Child,
        mut font: Font,
    ) -> Result<TerminalWindow, Error> {
        let (cell_height, cell_width, descender) = font.get_metrics()?;

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
            window,
            window_context,
            buffer_image,
            conn,
            width,
            height,
            font: RefCell::new(font),
            cell_height,
            cell_width,
            descender,
            terminal,
            pty,
            process,
            glyph_cache: RefCell::new(HashMap::new()),
        })
    }

    pub fn show(&self) {
        self.window.show();
    }

    pub fn resize_surfaces(&mut self, width: u16, height: u16) -> Result<bool, Error> {
        if width != self.width || height != self.height {
            debug!("resize {},{}", width, height);

            let mut buffer = BufferImage::new(
                self.conn,
                self.window.as_drawable(),
                width as usize,
                height as usize,
            );
            buffer.draw_image(0, 0, &self.buffer_image, xgfx::Operator::Source);
            self.buffer_image = buffer;

            self.width = width;
            self.height = height;

            let rows = height / self.cell_height as u16;
            let cols = width / self.cell_width as u16;
            self.pty.resize(rows, cols, width, height)?;
            self.terminal.resize(rows as usize, cols as usize);

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
                    &self.window,
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
    fn cached_glyph(&self, info: &GlyphInfo) -> Result<Rc<CachedGlyph>, Error> {
        let key = GlyphKey {
            font_idx: info.font_idx,
            glyph_pos: info.glyph_pos,
        };

        let mut cache = self.glyph_cache.borrow_mut();

        if let Some(entry) = cache.get(&key) {
            return Ok(Rc::clone(entry));
        }

        let glyph = self.load_glyph(info)?;
        cache.insert(key, Rc::clone(&glyph));
        Ok(glyph)
    }

    /// Perform the load and render of a glyph
    fn load_glyph(&self, info: &GlyphInfo) -> Result<Rc<CachedGlyph>, Error> {
        let (has_color, ft_glyph) = {
            let mut font = self.font.borrow_mut();
            let has_color = font.has_color(info.font_idx)?;
            // This clone is conceptually unsafe, but ok in practice as we are
            // single threaded and don't load any other glyphs in the body of
            // this load_glyph() function.
            let ft_glyph = font.load_glyph(info.font_idx, info.glyph_pos)?.clone();
            (has_color, ft_glyph)
        };

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
    fn shape_text(&self, s: &str) -> Result<Vec<GlyphInfo>, Error> {
        let mut font = self.font.borrow_mut();
        font.shape(0, s)
    }

    pub fn paint(&mut self) -> Result<(), Error> {
        let palette = term::color::ColorPalette::default();
        let background_color = palette.resolve(&term::color::ColorAttribute::Background);

        let cell_height = self.cell_height.ceil() as usize;
        let cell_width = self.cell_width.ceil() as usize;

        let cursor = self.terminal.cursor_pos();
        {
            let dirty_lines = self.terminal.get_dirty_lines();

            for (line_idx, line) in dirty_lines {

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

                    let (fg_color, bg_color) = {
                        let mut fg_color = &attrs.foreground;
                        let mut bg_color = &attrs.background;

                        if attrs.reverse() {
                            mem::swap(&mut fg_color, &mut bg_color);
                        }

                        (fg_color, bg_color)
                    };

                    let bg_color = palette.resolve(bg_color);

                    // Shape the printable text from this cluster
                    let glyph_info = self.shape_text(&cluster.text)?;
                    for info in glyph_info.iter() {
                        let cell_idx = cluster.byte_to_cell_idx[info.cluster as usize];
                        let cell = &line.cells[cell_idx];
                        let cell_print_width = cell.width();

                        // Render the cell background color
                        self.buffer_image.clear_rect(
                            x,
                            y,
                            cell_print_width * cell_width,
                            cell_height,
                            bg_color.into(),
                        );

                        // Render the cursor, if it overlaps with the current cluster
                        if line_idx as i64 == cursor.y {
                            for cur_x in cell_idx..cell_idx + info.num_cells as usize {
                                if cursor.x == cur_x {
                                    // The cursor fits in this cell, so render the cursor bg
                                    self.buffer_image.clear_rect(
                                        (cur_x * cell_width) as isize,
                                        y,
                                        cell_width * line.cells[cur_x].width(),
                                        cell_height,
                                        palette.cursor().into(),
                                    );
                                }
                            }
                        }

                        let glyph = self.cached_glyph(info)?;
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
                                let glyph_color = palette.resolve(glyph_color);
                                let operator = if glyph.has_color {
                                    xgfx::Operator::Over
                                } else {
                                    xgfx::Operator::MultiplyThenOver(glyph_color.into())
                                };

                                let slice_offset = slice_x * cell_width;

                                self.buffer_image.draw_image_subset(
                                    slice_offset as isize + x + glyph.x_offset as isize +
                                        glyph.bearing_x,
                                    y + cell_height as isize + self.descender -
                                        (glyph.y_offset as isize + glyph.bearing_y),
                                    slice_offset,
                                    0,
                                    (glyph_width - slice_offset).min(cell_width),
                                    glyph_height.min(cell_height),
                                    image,
                                    operator,
                                );

                                if is_cursor {
                                    // Render a block outline style of cursor.
                                    // TODO: make this respect user configuration
                                    self.buffer_image.draw_rect(
                                        (slice_offset + (cell_idx * cell_width)) as
                                            isize,
                                        y,
                                        // take care to use the print width here otherwise
                                        // the rectangle will incorrectly bisect the glyph
                                        (cell_print_width * cell_width),
                                        cell_height,
                                        palette.cursor().into(),
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
                            &self.window,
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
            match self.pty.read(&mut buf) {
                Ok(size) => {
                    for answer in self.terminal.advance_bytes(&buf[0..size]) {
                        match answer {
                            term::AnswerBack::WriteToPty(response) => {
                                self.pty.write(&response).ok(); // discard error
                            }
                            term::AnswerBack::TitleChanged(title) => {
                                self.window.set_title(&title);
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

    pub fn key_down(&mut self, event: &xcb::KeyPressEvent) -> Result<(), Error> {
        let (code, mods) = self.decode_key(event);
        // println!("Key pressed {:?} {:?}", code, mods);
        self.terminal.key_down(code, mods, &mut self.pty)
    }

    pub fn key_up(&mut self, event: &xcb::KeyPressEvent) -> Result<(), Error> {
        let (code, mods) = self.decode_key(event);
        // println!("Key released {:?} {:?}", code, mods);
        self.terminal.key_up(code, mods, &mut self.pty)
    }
}
