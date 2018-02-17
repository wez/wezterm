use config::TextStyle;
use euclid;
use failure::{self, Error};
use font::{FontConfiguration, GlyphInfo, ftwrap};
use glium::{self, Surface};
use pty::MasterPty;
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::mem;
use std::ops::{Deref, Range};
use std::process::Child;
use std::process::Command;
use std::rc::Rc;
use std::slice;
use term::{self, CursorPosition, KeyCode, KeyModifiers, Line, MouseButton, MouseEvent,
           MouseEventKind, TerminalHost, Underline};
use term::color::RgbColor;
use term::hyperlink::Hyperlink;
use xcb;
use xcb_util;
use xgfx::{self, Connection, Drawable};
use xkeysyms;

type Transform2D = euclid::Transform2D<f32>;
type Transform3D = euclid::Transform3D<f32>;

#[derive(Copy, Clone, Debug)]
struct Point(euclid::Point2D<f32>);

impl Deref for Point {
    type Target = euclid::Point2D<f32>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

unsafe impl glium::vertex::Attribute for Point {
    #[inline]
    fn get_type() -> glium::vertex::AttributeType {
        glium::vertex::AttributeType::F32F32
    }
}

impl Point {
    fn new(x: f32, y: f32) -> Self {
        Self { 0: euclid::point2(x, y) }
    }
}

#[derive(Copy, Clone, Debug)]
struct Vertex {
    position: Point,
    tex: [f32; 2],
}

implement_vertex!(Vertex, position, tex);

const VERTEX_SHADER: &str = r#"
#version 300 es
in vec2 position;
in vec2 tex;

uniform mat4 projection;
uniform mat4 translation;

out vec2 tex_coords;

void main() {
    tex_coords = tex;
    vec4 pos = vec4(position, 0.0, 1.0) * translation;
    gl_Position = projection * pos;
}
"#;

const FRAGMENT_SHADER: &str = r#"
#version 300 es
precision mediump float;
uniform vec3 fg_color;
uniform vec4 bg_color;
out vec4 color;
in vec2 tex_coords;
uniform sampler2D glyph_tex;
uniform bool has_color;
uniform bool bg_fill;

void main() {
    if (bg_fill) {
        color = bg_color;
    } else {
        color = texture2D(glyph_tex, tex_coords);
        if (!has_color) {
            // if it's not a color emoji, tint with the fg_color
            color = color * vec4(fg_color, 1.0);
        }
    }
}
"#;

const FILL_RECT_FRAG_SHADER: &str = r#"
#version 300 es
precision mediump float;
out vec4 color;
uniform vec4 bg_color;

void main() {
    color = bg_color;
}
"#;

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
    cell_height: usize,
    cell_width: usize,
    descender: isize,
    terminal: term::Terminal,
    process: Child,
    glyph_cache: RefCell<HashMap<GlyphKey, Rc<CachedGlyph>>>,
    palette: term::color::ColorPalette,
    program: glium::Program,
    fill_program: glium::Program,
    glyph_vertex_buffer: glium::VertexBuffer<Vertex>,
    line_vertex_buffer: glium::VertexBuffer<Vertex>,
    projection: Transform3D,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct GlyphKey {
    font_idx: usize,
    glyph_pos: u32,
    style: TextStyle,
}

/// Caches a rendered glyph.
/// The image data may be None for whitespace glyphs.
#[derive(Debug)]
struct CachedGlyph {
    has_color: bool,
    x_offset: isize,
    y_offset: isize,
    bearing_x: isize,
    bearing_y: isize,
    texture: Option<glium::texture::SrgbTexture2d>,
    scale: f32,
}

impl<'a> term::TerminalHost for Host<'a> {
    fn writer(&mut self) -> &mut Write {
        &mut self.pty
    }

    fn click_link(&mut self, link: &Rc<Hyperlink>) {
        // TODO: make this configurable
        let mut cmd = Command::new("xdg-open");
        cmd.arg(&link.url);
        match cmd.spawn() {
            Ok(_) => {}
            Err(err) => eprintln!("failed to spawn xdg-open {}: {:?}", link.url, err),
        }
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

    fn set_title(&mut self, title: &str) {
        self.window.set_title(title);
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
        palette: term::color::ColorPalette,
    ) -> Result<TerminalWindow, Error> {
        let (cell_height, cell_width, descender) = {
            // Urgh, this is a bit repeaty, but we need to satisfy the borrow checker
            let font = fonts.default_font()?;
            let tuple = font.borrow_mut().get_metrics()?;
            tuple
        };

        let window = xgfx::Window::new(&conn, width, height)?;
        window.set_title("wezterm");

        let descender = if descender.is_positive() {
            ((descender as f64) / 64.0).ceil() as isize
        } else {
            ((descender as f64) / 64.0).floor() as isize
        };

        let host = Host {
            window,
            pty,
            timestamp: 0,
            clipboard: None,
        };
        let cell_height = cell_height.ceil() as usize;
        let cell_width = cell_width.ceil() as usize;

        let ch = cell_height as f32;
        let cw = cell_width as f32;

        let line_vertex_buffer = {
            let shape = [
                Vertex {
                    position: Point::new(0.0, 0.0),
                    tex: [0.0, 0f32],
                },
                Vertex {
                    position: Point::new(cw, 0.0),
                    tex: [0.0, 0f32],
                },
            ];
            glium::VertexBuffer::new(&host.window, &shape)?
        };

        let glyph_vertex_buffer = {
            let top_left = Vertex {
                position: Point::new(0.0, 0.0),
                tex: [0.0, 0f32],
            };
            let top_right = Vertex {
                position: Point::new(cw, 0.0),
                tex: [1.0, 0f32],
            };
            let bot_left = Vertex {
                position: Point::new(0.0, ch),
                tex: [0.0, 1.0f32],
            };
            let bot_right = Vertex {
                position: Point::new(cw, ch),
                tex: [1.0, 1.0f32],
            };
            let shape = [top_left, top_right, bot_left, bot_right];
            glium::VertexBuffer::new(&host.window, &shape)?
        };

        let program =
            glium::Program::from_source(&host.window, VERTEX_SHADER, FRAGMENT_SHADER, None)?;

        let fill_program =
            glium::Program::from_source(&host.window, VERTEX_SHADER, FILL_RECT_FRAG_SHADER, None)?;

        Ok(TerminalWindow {
            host,
            program,
            fill_program,
            glyph_vertex_buffer,
            line_vertex_buffer,
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
            palette,
            projection: Self::compute_projection(width as f32, height as f32),
        })
    }

    pub fn show(&self) {
        self.host.window.show();
    }

    fn compute_projection(width: f32, height: f32) -> Transform3D {
        // The projection corrects for the aspect ratio and flips the y-axis
        Transform3D::ortho(
            -width / 2.0,
            width / 2.0,
            height / 2.0,
            -height / 2.0,
            -1.0,
            1.0,
        )
    }

    pub fn resize_surfaces(&mut self, width: u16, height: u16) -> Result<bool, Error> {
        if width != self.width || height != self.height {
            debug!("resize {},{}", width, height);

            self.width = width;
            self.height = height;
            self.projection = Self::compute_projection(width as f32, height as f32);

            // The +1 in here is to handle an irritating case.
            // When we get N rows with a gap of cell_height - 1 left at
            // the bottom, we can usually squeeze that extra row in there,
            // so optimistically pretend that we have that extra pixel!
            let rows = ((height as usize + 1) / self.cell_height) as u16;
            let cols = ((width as usize + 1) / self.cell_width) as u16;
            self.host.pty.resize(rows, cols, width, height)?;
            self.terminal.resize(rows as usize, cols as usize);

            Ok(true)
        } else {
            debug!("ignoring extra resize");
            Ok(false)
        }
    }

    pub fn expose(&mut self, _x: u16, _y: u16, _width: u16, _height: u16) -> Result<(), Error> {
        self.paint()
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
        let (x_offset, y_offset) = if scale != 1.0 {
            (info.x_offset * scale, info.y_offset * scale)
        } else {
            (info.x_offset, info.y_offset)
        };

        let glyph = if ft_glyph.bitmap.width == 0 || ft_glyph.bitmap.rows == 0 {
            // a whitespace glyph
            CachedGlyph {
                texture: None,
                has_color,
                x_offset: x_offset as isize,
                y_offset: y_offset as isize,
                bearing_x: 0,
                bearing_y: 0,
                scale: scale as f32,
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


            let raw_im = match mode {
                ftwrap::FT_Pixel_Mode::FT_PIXEL_MODE_LCD => {
                    let width = ft_glyph.bitmap.width as usize / 3;
                    let height = ft_glyph.bitmap.rows as usize;
                    let size = (width * height * 4) as usize;
                    let mut rgba = Vec::with_capacity(size);
                    rgba.resize(size, 0u8);
                    for y in 0..height {
                        let src_offset = y * pitch as usize;
                        let dest_offset = y * width * 4;
                        for x in 0..width {
                            let blue = data[src_offset + (x * 3) + 0];
                            let green = data[src_offset + (x * 3) + 1];
                            let red = data[src_offset + (x * 3) + 2];
                            let alpha = red | green | blue;
                            rgba[dest_offset + (x * 4) + 0] = red;
                            rgba[dest_offset + (x * 4) + 1] = green;
                            rgba[dest_offset + (x * 4) + 2] = blue;
                            rgba[dest_offset + (x * 4) + 3] = alpha;
                        }
                    }

                    glium::texture::RawImage2d::from_raw_rgba(rgba, (width as u32, height as u32))
                }
                ftwrap::FT_Pixel_Mode::FT_PIXEL_MODE_BGRA => {
                    let width = ft_glyph.bitmap.width as usize;
                    let height = ft_glyph.bitmap.rows as usize;
                    let size = (width * height * 4) as usize;
                    let mut rgba = Vec::with_capacity(size);
                    rgba.resize(size, 0u8);
                    for y in 0..height {
                        let src_offset = y * pitch as usize;
                        let dest_offset = y * width * 4;
                        for x in 0..width {
                            let blue = data[src_offset + (x * 4) + 0];
                            let green = data[src_offset + (x * 4) + 1];
                            let red = data[src_offset + (x * 4) + 2];
                            let alpha = data[src_offset + (x * 4) + 3];

                            rgba[dest_offset + (x * 4) + 0] = red;
                            rgba[dest_offset + (x * 4) + 1] = green;
                            rgba[dest_offset + (x * 4) + 2] = blue;
                            rgba[dest_offset + (x * 4) + 3] = alpha;
                        }
                    }

                    glium::texture::RawImage2d::from_raw_rgba(rgba, (width as u32, height as u32))
                }
                ftwrap::FT_Pixel_Mode::FT_PIXEL_MODE_GRAY => {
                    let width = ft_glyph.bitmap.width as usize;
                    let height = ft_glyph.bitmap.rows as usize;
                    let size = (width * height * 4) as usize;
                    let mut rgba = Vec::with_capacity(size);
                    rgba.resize(size, 0u8);
                    for y in 0..height {
                        let src_offset = y * pitch;
                        let dest_offset = y * width * 4;
                        for x in 0..width {
                            let gray = data[src_offset + x];

                            rgba[dest_offset + (x * 4) + 0] = gray;
                            rgba[dest_offset + (x * 4) + 1] = gray;
                            rgba[dest_offset + (x * 4) + 2] = gray;
                            rgba[dest_offset + (x * 4) + 3] = gray;
                        }
                    }
                    glium::texture::RawImage2d::from_raw_rgba(rgba, (width as u32, height as u32))
                }
                mode @ _ => bail!("unhandled pixel mode: {:?}", mode),
            };

            let tex = glium::texture::SrgbTexture2d::new(&self.host.window, raw_im)?;

            let bearing_x = (ft_glyph.bitmap_left as f64 * scale) as isize;
            let bearing_y = (ft_glyph.bitmap_top as f64 * scale) as isize;

            CachedGlyph {
                texture: Some(tex),
                has_color,
                x_offset: x_offset as isize,
                y_offset: y_offset as isize,
                bearing_x,
                bearing_y,
                scale: scale as f32,
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

    fn fill_rect(
        &self,
        target: &mut glium::Frame,
        x: isize,
        y: isize,
        num_cells_wide: u32,
        num_cells_high: u32,
        color: RgbColor,
    ) -> Result<(), Error> {
        // Translate cell coordinate from top-left origin in cell coords
        // to center origin pixel coords
        let xlate_model = Transform2D::create_translation(
            x as f32 - self.width as f32 / 2.0,
            y as f32 - self.height as f32 / 2.0,
        ).to_3d();
        let scale_model = Transform2D::create_scale(num_cells_wide as f32, num_cells_high as f32)
            .to_3d();

        target.draw(
            &self.glyph_vertex_buffer,
            glium::index::NoIndices(
                glium::index::PrimitiveType::TriangleStrip,
            ),
            &self.fill_program,
            &uniform! {
                    projection: self.projection.to_column_arrays(),
                    translation: scale_model.post_mul(&xlate_model).to_column_arrays(),
                    bg_color: color.to_linear_tuple_rgba(),
                },
            &glium::DrawParameters {
                blend: glium::Blend::alpha_blending(),
                //dithering: false,
                ..Default::default()
            },
        )?;

        Ok(())
    }

    /// Render a line strike through the glyph at the given coords.
    fn render_strikethrough(
        &self,
        target: &mut glium::Frame,
        x: isize,
        cell_top: isize,
        baseline: isize,
        num_cells_wide: u8,
        glyph_color: RgbColor,
    ) -> Result<(), Error> {
        self.draw_line(
            target,
            x,
            cell_top + (baseline - cell_top) / 2,
            num_cells_wide,
            glyph_color,
        )?;
        Ok(())
    }

    fn draw_line(
        &self,
        target: &mut glium::Frame,
        x: isize,
        y: isize,
        num_cells_wide: u8,
        color: RgbColor,
    ) -> Result<(), Error> {
        // Translate cell coordinate from top-left origin in cell coords
        // to center origin pixel coords
        let xlate_model = Transform2D::create_translation(
            x as f32 - self.width as f32 / 2.0,
            y as f32 - self.height as f32 / 2.0,
        ).to_3d();
        let scale_model = Transform2D::create_scale(num_cells_wide as f32, 1.0).to_3d();

        target.draw(
            &self.line_vertex_buffer,
            glium::index::NoIndices(
                glium::index::PrimitiveType::LinesList,
            ),
            &self.fill_program,
            &uniform! {
                    projection: self.projection.to_column_arrays(),
                    translation: scale_model.post_mul(&xlate_model).to_column_arrays(),
                    bg_color: color.to_linear_tuple_rgba(),
                },
            &glium::DrawParameters {
                blend: glium::Blend::alpha_blending(),
                line_width: Some(1.0),
                ..Default::default()
            },
        )?;

        Ok(())
    }

    /// Render a specific style of underline at the given coords.
    fn render_underline(
        &self,
        target: &mut glium::Frame,
        x: isize,
        baseline: isize,
        num_cells_wide: u8,
        style: Underline,
        glyph_color: RgbColor,
    ) -> Result<(), Error> {
        match style {
            Underline::None => {}
            Underline::Single => {
                self.draw_line(
                    target,
                    x,
                    baseline + 2,
                    num_cells_wide,
                    glyph_color,
                )?;
            }
            Underline::Double => {
                self.draw_line(
                    target,
                    x,
                    baseline + 1,
                    num_cells_wide,
                    glyph_color,
                )?;
                self.draw_line(
                    target,
                    x,
                    baseline + 3,
                    num_cells_wide,
                    glyph_color,
                )?;
            }
        }
        Ok(())
    }

    fn render_glyph(
        &self,
        target: &mut glium::Frame,
        x: isize,
        base_y: isize,
        glyph: &Rc<CachedGlyph>,
        image: &glium::texture::SrgbTexture2d,
        metric_width: usize,
        glyph_color: RgbColor,
        bg_color: RgbColor,
    ) -> Result<(), Error> {
        let width = self.width as f32;
        let height = self.height as f32;

        let (glyph_width, glyph_height) = {
            let (w, h) = image.dimensions();
            (w as usize, h as usize)
        };

        let scale_y = glyph.scale * glyph_height as f32 / self.cell_height as f32;
        let scale_x = glyph.scale * glyph_width as f32 / metric_width as f32;

        let draw_y = base_y - (glyph.y_offset as isize + glyph.bearing_y);
        let draw_x = x + glyph.x_offset as isize + glyph.bearing_x;

        // Translate cell coordinate from top-left origin in cell coords
        // to center origin pixel coords
        let xlate_model = Transform2D::create_translation(
            (draw_x as f32) - width / 2.0,
            (draw_y as f32) - height / 2.0,
        ).to_3d();

        let scale_model = Transform2D::create_scale(scale_x, scale_y).to_3d();

        target.draw(
            &self.glyph_vertex_buffer,
            glium::index::NoIndices(
                glium::index::PrimitiveType::TriangleStrip,
            ),
            &self.program,
            &uniform! {
                    fg_color: glyph_color.to_linear_tuple_rgb(),
                    projection: self.projection.to_column_arrays(),
                    translation: scale_model.post_mul(&xlate_model).to_column_arrays(),
                    glyph_tex: image,
                    has_color: glyph.has_color,
                    bg_color: bg_color.to_linear_tuple_rgba(),
                    bg_fill: false,
                },
            &glium::DrawParameters {
                blend: glium::Blend::alpha_blending(),
                dithering: false,
                ..Default::default()
            },
        )?;

        Ok(())
    }

    fn render_line(
        &self,
        target: &mut glium::Frame,
        line_idx: usize,
        line: &Line,
        selection: Range<usize>,
        cursor: &CursorPosition,
    ) -> Result<(), Error> {

        let mut x = 0 as isize;
        let y = (line_idx * self.cell_height) as isize;
        let base_y = y + self.cell_height as isize + self.descender;

        let current_highlight = self.terminal.current_highlight();

        // Break the line into clusters of cells with the same attributes
        let cell_clusters = line.cluster();
        for cluster in cell_clusters {
            let attrs = &cluster.attrs;
            let is_highlited_hyperlink = match (&attrs.hyperlink, &current_highlight) {
                (&Some(ref this), &Some(ref highlight)) => this == highlight,
                _ => false,
            };
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

                let cluster_width = info.num_cells as usize * metric_width;

                // Render the cluster background color
                self.fill_rect(
                    target,
                    x,
                    y,
                    info.num_cells as u32,
                    1,
                    bg_color,
                )?;

                // Render selection background
                for cur_x in cell_idx..cell_idx + info.num_cells as usize {
                    if term::in_range(cur_x, &selection) {
                        self.fill_rect(
                            target,
                            (cur_x * metric_width) as isize,
                            y,
                            line.cells[cur_x].width() as u32,
                            1,
                            self.palette.cursor(),
                        )?;
                    }
                }

                // Render the cursor, if it overlaps with the current cluster
                if line_idx as i64 == cursor.y {
                    for cur_x in cell_idx..cell_idx + info.num_cells as usize {
                        if cursor.x == cur_x {
                            // The cursor fits in this cell, so render the cursor bg
                            self.fill_rect(
                                target,
                                (cur_x * metric_width) as isize,
                                y,
                                line.cells[cur_x].width() as u32,
                                1,
                                self.palette.cursor(),
                            )?;
                        }
                    }
                }

                let glyph = self.cached_glyph(info, &style)?;

                let glyph_color = match fg_color {
                    &term::color::ColorAttribute::Foreground => {
                        if let Some(fg) = style.foreground {
                            fg
                        } else {
                            self.palette.resolve(fg_color)
                        }
                    }
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
                    _ => self.palette.resolve(fg_color),
                };

                // glyph.image.is_none() for whitespace glyphs
                if let &Some(ref texture) = &glyph.texture {
                    self.render_glyph(
                        target,
                        x,
                        base_y,
                        &glyph,
                        texture,
                        metric_width,
                        glyph_color,
                        bg_color,
                    )?;
                }

                // Figure out what we're going to draw for the underline.
                // If the current cell is part of the current URL highlight
                // then we want to show the underline.  If that text is already
                // underlined we pick a different color for the underline to
                // make it more distinct.
                // TODO: make that highlight underline color configurable.
                let (underline, under_color) = match (is_highlited_hyperlink, attrs.underline()) {
                    (true, Underline::None) => (Underline::Single, glyph_color),
                    (true, Underline::Single) => (Underline::Single, self.palette.cursor()),
                    (true, Underline::Double) => (Underline::Single, self.palette.cursor()),
                    (false, underline) => (underline, glyph_color),
                };

                self.render_underline(
                    target,
                    x,
                    base_y,
                    info.num_cells,
                    underline,
                    under_color,
                )?;

                if attrs.strikethrough() {
                    self.render_strikethrough(
                        target,
                        x,
                        y,
                        base_y,
                        info.num_cells,
                        glyph_color.into(),
                    )?;
                }

                // Always advance by our computed metric, despite what the shaping info
                // says, otherwise we tend to end up with very slightly offset cells
                // for example in vim when the window is split vertically.
                x += cluster_width as isize;
            }
        }

        Ok(())
    }

    pub fn paint(&mut self) -> Result<(), Error> {

        let mut target = self.host.window.draw();

        let background_color = self.palette.resolve(
            &term::color::ColorAttribute::Background,
        );
        let (r, g, b, a) = background_color.to_linear_tuple_rgba();
        target.clear_color(r, g, b, a);

        let cursor = self.terminal.cursor_pos();
        {
            let dirty_lines = self.terminal.get_dirty_lines(true);

            for (line_idx, line, selrange) in dirty_lines {
                self.render_line(
                    &mut target,
                    line_idx,
                    line,
                    selrange,
                    &cursor,
                )?;
            }
        }

        self.terminal.clean_dirty_lines();
        target.finish().unwrap();

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
                    self.terminal.advance_bytes(&buf[0..size], &mut self.host);
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
                    x: (motion.event_x() as usize / self.cell_width) as usize,
                    y: (motion.event_y() as usize / self.cell_height) as i64,
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
                    x: (button_press.event_x() as usize / self.cell_width) as usize,
                    y: (button_press.event_y() as usize / self.cell_height) as i64,
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
