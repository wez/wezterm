use config::TextStyle;
use euclid;
use failure::{self, Error};
use font::{FontConfiguration, GlyphInfo, ftwrap};
use glium::{self, IndexBuffer, Surface, VertexBuffer};
use glium::texture::SrgbTexture2d;
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
use term::hyperlink::Hyperlink;
use xcb;
use xcb_util;
use xgfx::{self, Connection, Drawable};
use xkeysyms;

use textureatlas::{Atlas, Sprite, SpriteSlice};

type Transform3D = euclid::Transform3D<f32>;

#[derive(Copy, Clone, Debug)]
struct Point(euclid::Point2D<f32>);

impl Default for Point {
    fn default() -> Point {
        Point::new(0.0, 0.0)
    }
}

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

/// Each cell is composed of two triangles built from 4 vertices.
/// The buffer is organized row by row.
const VERTICES_PER_CELL: usize = 4;
const V_TOP_LEFT: usize = 0;
const V_TOP_RIGHT: usize = 1;
const V_BOT_LEFT: usize = 2;
const V_BOT_RIGHT: usize = 3;

#[derive(Copy, Clone, Debug, Default)]
struct Vertex {
    // pre-computed by compute_vertices and changed only on resize
    position: Point,
    // adjustment for glyph size, recomputed each time the cell changes
    adjust: Point,
    // texture coords are updated as the screen contents change
    tex: (f32, f32),
    // cell foreground and background color
    fg_color: (f32, f32, f32, f32),
    bg_color: (f32, f32, f32, f32),
    /// Nominally a boolean, but the shader compiler hated it
    has_color: f32,
    /// Count of how many underlines there are
    underline: f32,
    strikethrough: f32,
    v_idx: f32,
}

implement_vertex!(
    Vertex,
    position,
    adjust,
    tex,
    fg_color,
    bg_color,
    has_color,
    underline,
    strikethrough,
    v_idx,
);

const VERTEX_SHADER: &str = r#"
#version 300 es
in vec2 position;
in vec2 adjust;
in vec2 tex;
in vec4 fg_color;
in vec4 bg_color;
in float has_color;
in float underline;
in float v_idx;

uniform mat4 projection;
uniform mat4 translation;
uniform bool bg_fill;
uniform bool underlining;

out vec2 tex_coords;
out vec4 o_fg_color;
out vec4 o_bg_color;
out float o_has_color;
out float o_underline;

// Offset from the RHS texture coordinate to the LHS.
// This is an underestimation to avoid the shader interpolating
// the underline gylph into its neighbor.
const float underline_offset = (1.0 / 5.0);

void main() {
    o_fg_color = fg_color;
    o_bg_color = bg_color;
    o_has_color = has_color;
    o_underline = underline;

    if (bg_fill || underlining) {
        gl_Position = projection * vec4(position, 0.0, 1.0);

        if (underlining) {
            // Populate the underline texture coordinates based on the
            // v_idx (which tells us which corner of the cell we're
            // looking at) and o_underline which corresponds to one
            // of the U_XXX constants defined in the rust code below
            // and which holds the RHS position in the texture coordinate
            // space for the underline texture layer.
            if (v_idx == 0.0) { // top left
                tex_coords = vec2(o_underline - underline_offset, -1.0);
            } else if (v_idx == 1.0) { // top right
                tex_coords = vec2(o_underline, -1.0);
            } else if (v_idx == 2.0) { // bot left
                tex_coords = vec2(o_underline- underline_offset, 0.0);
            } else { // bot right
                tex_coords = vec2(o_underline, 0.0);
            }
        }

    } else {
        gl_Position = projection * vec4(position + adjust, 0.0, 1.0);
        tex_coords = tex;
    }
}
"#;

/// How many columns the underline texture has
const U_COLS: f32 = 5.0;
/// The glyph has no underline or strikethrough
const U_NONE: f32 = 0.0;
/// The glyph has a single underline.  This value is actually the texture
/// coordinate for the right hand side of the underline.
const U_ONE: f32 = 1.0 / U_COLS;
/// Texture coord for the RHS of the double underline glyph
const U_TWO: f32 = 2.0 / U_COLS;
/// Texture coord for the RHS of the strikethrough glyph
const U_STRIKE: f32 = 3.0 / U_COLS;
/// Texture coord for the RHS of the strikethrough + single underline glyph
const U_STRIKE_ONE: f32 = 4.0 / U_COLS;
/// Texture coord for the RHS of the strikethrough + double underline glyph
const U_STRIKE_TWO: f32 = 5.0 / U_COLS;

const FRAGMENT_SHADER: &str = r#"
#version 300 es
precision mediump float;
in vec2 tex_coords;
in vec4 o_fg_color;
in vec4 o_bg_color;
in float o_has_color;
in float o_underline;

out vec4 color;
uniform sampler2D glyph_tex;
uniform sampler2D underline_tex;
uniform bool bg_fill;
uniform bool underlining;

float multiply_one(float src, float dst, float inv_dst_alpha, float inv_src_alpha) {
    return (src * dst) + (src * (inv_dst_alpha)) + (dst * (inv_src_alpha));
}

// Alpha-regulated multiply to colorize the glyph bitmap
vec4 multiply(vec4 src, vec4 dst) {
    float inv_src_alpha = 1.0 - src.a;
    float inv_dst_alpha = 1.0 - dst.a;

    return vec4(
        multiply_one(src.r, dst.r, inv_dst_alpha, inv_src_alpha),
        multiply_one(src.g, dst.g, inv_dst_alpha, inv_src_alpha),
        multiply_one(src.b, dst.b, inv_dst_alpha, inv_src_alpha),
        dst.a);
}

void main() {
    if (bg_fill) {
        color = o_bg_color;
    } else if (underlining) {
        if (o_underline != 0.0) {
            color = texture2D(underline_tex, tex_coords) * o_fg_color;
        } else {
            discard;
        }
    } else {
        color = texture2D(glyph_tex, tex_coords);
        if (o_has_color == 0.0) {
            // if it's not a color emoji, tint with the fg_color
            color = multiply(o_fg_color, color);
        }
    }
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
    glyph_vertex_buffer: RefCell<VertexBuffer<Vertex>>,
    glyph_index_buffer: IndexBuffer<u32>,
    projection: Transform3D,
    atlas: RefCell<Atlas>,
    underline_tex: SrgbTexture2d,
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
    texture: Option<Sprite>,
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

        // Create the texture atlas for the line decoration layer.
        // This is a bitmap with columns to accomodate the U_XXX
        // constants defined above.
        let underline_tex = {
            let width = 5 * cell_width;
            let mut underline_data = Vec::with_capacity(width * cell_height * 4);
            underline_data.resize(width * cell_height * 4, 0u8);

            let descender_row = (cell_height as isize + descender) as usize;
            let strike_row = descender_row / 2;

            // First, the single underline.
            // We place this as the descender position.
            {
                let col = 0;
                let offset = ((width * 4) * descender_row) + (col * 4 * cell_width);
                for i in 0..4 * cell_width {
                    underline_data[offset + i] = 0xff;
                }
            }
            // Double underline,
            // We place this just above and below the descender
            {
                let col = 1;
                let offset_one = ((width * 4) * (descender_row - 1)) + (col * 4 * cell_width);
                let offset_two = ((width * 4) * (descender_row + 1)) + (col * 4 * cell_width);
                for i in 0..4 * cell_width {
                    underline_data[offset_one + i] = 0xff;
                    underline_data[offset_two + i] = 0xff;
                }
            }
            // Strikethrough
            {
                let col = 2;
                let offset = (width * 4) * strike_row + (col * 4 * cell_width);
                for i in 0..4 * cell_width {
                    underline_data[offset + i] = 0xff;
                }
            }
            // Strikethrough and single underline
            {
                let col = 3;
                let offset_one = ((width * 4) * descender_row) + (col * 4 * cell_width);
                let offset_two = ((width * 4) * strike_row) + (col * 4 * cell_width);
                for i in 0..4 * cell_width {
                    underline_data[offset_one + i] = 0xff;
                    underline_data[offset_two + i] = 0xff;
                }
            }
            // Strikethrough and double underline
            {
                let col = 4;
                let offset_one = ((width * 4) * (descender_row - 1)) + (col * 4 * cell_width);
                let offset_two = ((width * 4) * strike_row) + (col * 4 * cell_width);
                let offset_three = ((width * 4) * (descender_row + 1)) + (col * 4 * cell_width);
                for i in 0..4 * cell_width {
                    underline_data[offset_one + i] = 0xff;
                    underline_data[offset_two + i] = 0xff;
                    underline_data[offset_three + i] = 0xff;
                }
            }

            glium::texture::SrgbTexture2d::new(
                &host.window,
                glium::texture::RawImage2d::from_raw_rgba(
                    underline_data,
                    (width as u32, cell_height as u32),
                ),
            )?
        };

        let (glyph_vertex_buffer, glyph_index_buffer) = Self::compute_vertices(
            &host,
            cell_width as f32,
            cell_height as f32,
            width as f32,
            height as f32,
        )?;

        let program =
            glium::Program::from_source(&host.window, VERTEX_SHADER, FRAGMENT_SHADER, None)?;

        let atlas = RefCell::new(Atlas::new(&host.window)?);

        Ok(TerminalWindow {
            host,
            atlas,
            program,
            glyph_vertex_buffer: RefCell::new(glyph_vertex_buffer),
            glyph_index_buffer,
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
            underline_tex,
        })
    }

    pub fn show(&self) {
        self.host.window.show();
    }

    /// Compute a vertex buffer to hold the quads that comprise the visible
    /// portion of the screen.   We recreate this when the screen is resized.
    /// The idea is that we want to minimize and heavy lifting and computation
    /// and instead just poke some attributes into the offset that corresponds
    /// to a changed cell when we need to repaint the screen, and then just
    /// let the GPU figure out the rest.
    fn compute_vertices(
        host: &Host,
        cell_width: f32,
        cell_height: f32,
        width: f32,
        height: f32,
    ) -> Result<(VertexBuffer<Vertex>, IndexBuffer<u32>), Error> {
        let mut verts = Vec::new();
        let mut indices = Vec::new();

        let num_cols = ((width + 1.0) / cell_width).floor() as usize;
        let num_rows = ((height + 1.0) / cell_height).floor() as usize;

        for y in 0..num_rows {
            for x in 0..num_cols {
                let y_pos = (height / -2.0) + (y as f32 * cell_height);
                let x_pos = (width / -2.0) + (x as f32 * cell_width);
                // Remember starting index for this position
                let idx = verts.len() as u32;
                verts.push(Vertex {
                    // Top left
                    position: Point::new(x_pos, y_pos),
                    v_idx: V_TOP_LEFT as f32,
                    ..Default::default()
                });
                verts.push(Vertex {
                    // Top Right
                    position: Point::new(x_pos + cell_width, y_pos),
                    v_idx: V_TOP_RIGHT as f32,
                    ..Default::default()
                });
                verts.push(Vertex {
                    // Bottom Left
                    position: Point::new(x_pos, y_pos + cell_height),
                    v_idx: V_BOT_LEFT as f32,
                    ..Default::default()
                });
                verts.push(Vertex {
                    // Bottom Right
                    position: Point::new(x_pos + cell_width, y_pos + cell_height),
                    v_idx: V_BOT_RIGHT as f32,
                    ..Default::default()
                });

                // Emit two triangles to form the glyph quad
                indices.push(idx);
                indices.push(idx + 1);
                indices.push(idx + 2);
                indices.push(idx + 1);
                indices.push(idx + 2);
                indices.push(idx + 3);
            }
        }

        Ok((
            VertexBuffer::dynamic(&host.window, &verts)?,
            IndexBuffer::new(
                &host.window,
                glium::index::PrimitiveType::TrianglesList,
                &indices,
            )?,
        ))
    }

    /// The projection corrects for the aspect ratio and flips the y-axis
    fn compute_projection(width: f32, height: f32) -> Transform3D {
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

            let (glyph_vertex_buffer, glyph_index_buffer) = Self::compute_vertices(
                &self.host,
                self.cell_width as f32,
                self.cell_height as f32,
                width as f32,
                height as f32,
            )?;
            self.glyph_vertex_buffer = RefCell::new(glyph_vertex_buffer);
            self.glyph_index_buffer = glyph_index_buffer;

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
                ftwrap::FT_Pixel_Mode::FT_PIXEL_MODE_MONO => {
                    let width = ft_glyph.bitmap.width as usize;
                    let height = ft_glyph.bitmap.rows as usize;
                    let size = (width * height * 4) as usize;
                    let mut rgba = Vec::with_capacity(size);
                    rgba.resize(size, 0u8);
                    for y in 0..height {
                        let src_offset = y * pitch;
                        let dest_offset = y * width * 4;
                        let mut x = 0;
                        for i in 0..pitch {
                            if x >= width {
                                break;
                            }
                            let mut b = data[src_offset + i];
                            for _ in 0..8 {
                                if x >= width {
                                    break;
                                }
                                if b & 0x80 == 0x80 {
                                    for j in 0..4 {
                                        rgba[dest_offset + (x * 4) + j] = 0xff;
                                    }
                                }
                                b = b << 1;
                                x += 1;
                            }
                        }
                    }
                    glium::texture::RawImage2d::from_raw_rgba(rgba, (width as u32, height as u32))
                }
                mode @ _ => bail!("unhandled pixel mode: {:?}", mode),
            };

            let tex = self.atlas.borrow_mut().allocate(
                &self.host.window,
                raw_im.width,
                raw_im.height,
                raw_im,
            )?;

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

    /// "Render" a line of the terminal screen into the vertex buffer.
    /// This is nominally a matter of setting the fg/bg color and the
    /// texture coordinates for a given glyph.  There's a little bit
    /// of extra complexity to deal with multi-cell glyphs.
    fn render_screen_line(
        &self,
        line_idx: usize,
        line: &Line,
        selection: Range<usize>,
        cursor: &CursorPosition,
    ) -> Result<(), Error> {
        let num_cols = self.terminal.screen().physical_cols;
        let mut vb = self.glyph_vertex_buffer.borrow_mut();
        let mut vertices = {
            let per_line = num_cols * VERTICES_PER_CELL;
            let start = line_idx * per_line;
            vb.slice_mut(start..start + per_line)
                .ok_or_else(|| format_err!("we're confused about the screen size"))?
                .map()
        };

        let current_highlight = self.terminal.current_highlight();
        let cell_width = self.cell_width as f32;
        let cell_height = self.cell_height as f32;

        // Break the line into clusters of cells with the same attributes
        let cell_clusters = line.cluster();
        for cluster in cell_clusters {
            let attrs = &cluster.attrs;
            let is_highlited_hyperlink = match (&attrs.hyperlink, &current_highlight) {
                (&Some(ref this), &Some(ref highlight)) => this == highlight,
                _ => false,
            };
            let style = self.fonts.match_style(attrs);

            let (fg_color, bg_color) = {
                let mut fg_color = &attrs.foreground;
                let mut bg_color = &attrs.background;

                if attrs.reverse() {
                    mem::swap(&mut fg_color, &mut bg_color);
                }

                (fg_color, bg_color)
            };

            let bg_color = self.palette.resolve(bg_color).to_linear_tuple_rgba();

            // Shape the printable text from this cluster
            let glyph_info = self.shape_text(&cluster.text, &style)?;
            for info in glyph_info.iter() {
                let cell_idx = cluster.byte_to_cell_idx[info.cluster as usize];
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
                }.to_linear_tuple_rgba();

                let glyph_color = match &glyph.texture {
                    &Some(_) => glyph_color,
                    // Whitespace glyph; render with 0 alpha
                    &None => (0.0, 0.0, 0.0, 0.0f32),
                };

                let left: f32 = glyph.x_offset as f32 + glyph.bearing_x as f32;
                let top = (self.cell_height as f32 + self.descender as f32) -
                    (glyph.y_offset as f32 + glyph.bearing_y as f32);

                // underline and strikethrough
                // Figure out what we're going to draw for the underline.
                // If the current cell is part of the current URL highlight
                // then we want to show the underline.
                let underline: f32 = match (
                    is_highlited_hyperlink,
                    attrs.strikethrough(),
                    attrs.underline(),
                ) {
                    (true, false, Underline::None) => U_ONE,
                    (true, false, Underline::Single) => U_TWO,
                    (true, false, Underline::Double) => U_ONE,
                    (true, true, Underline::None) => U_STRIKE_ONE,
                    (true, true, Underline::Single) => U_STRIKE_TWO,
                    (true, true, Underline::Double) => U_STRIKE_ONE,
                    (false, false, Underline::None) => U_NONE,
                    (false, false, Underline::Single) => U_ONE,
                    (false, false, Underline::Double) => U_TWO,
                    (false, true, Underline::None) => U_STRIKE,
                    (false, true, Underline::Single) => U_STRIKE_ONE,
                    (false, true, Underline::Double) => U_STRIKE_TWO,
                };

                // Iterate each cell that comprises this glyph.  There is usually
                // a single cell per glyph but combining characters, ligatures
                // and emoji can be 2 or more cells wide.
                for glyph_idx in 0..info.num_cells as usize {
                    let cell_idx = cell_idx + glyph_idx;

                    if cell_idx >= num_cols {
                        // terminal line data is wider than the window.
                        // This happens for example while live resizing the window
                        // smaller than the terminal.
                        break;
                    }

                    let selected = term::in_range(cell_idx, &selection);
                    let is_cursor = line_idx as i64 == cursor.y && cursor.x == cell_idx;

                    let (glyph_color, bg_color) = match (selected, is_cursor) {
                        // Normally, render the cell as configured
                        (false, false) => (glyph_color, bg_color),
                        // Cursor cell always renders with background over cursor color
                        (_, true) => (
                            self.palette.background.to_linear_tuple_rgba(),
                            self.palette.cursor.to_linear_tuple_rgba(),
                        ),
                        // Selection text colors the background
                        (true, false) => (
                            glyph_color,
                            // TODO: configurable selection color
                            self.palette.cursor.to_linear_tuple_rgba(),
                        ),
                    };

                    let vert_idx = cell_idx * VERTICES_PER_CELL;
                    let vert = &mut vertices[vert_idx..vert_idx + VERTICES_PER_CELL];

                    vert[V_TOP_LEFT].fg_color = glyph_color;
                    vert[V_TOP_RIGHT].fg_color = glyph_color;
                    vert[V_BOT_LEFT].fg_color = glyph_color;
                    vert[V_BOT_RIGHT].fg_color = glyph_color;

                    vert[V_TOP_LEFT].bg_color = bg_color;
                    vert[V_TOP_RIGHT].bg_color = bg_color;
                    vert[V_BOT_LEFT].bg_color = bg_color;
                    vert[V_BOT_RIGHT].bg_color = bg_color;

                    vert[V_TOP_LEFT].underline = underline;
                    vert[V_TOP_RIGHT].underline = underline;
                    vert[V_BOT_LEFT].underline = underline;
                    vert[V_BOT_RIGHT].underline = underline;

                    match &glyph.texture {
                        &Some(ref texture) => {
                            let slice = SpriteSlice {
                                cell_idx: glyph_idx,
                                num_cells: info.num_cells as usize,
                                cell_width: self.cell_width,
                                scale: glyph.scale,
                                left_offset: left as i32,
                            };

                            // How much of the width of this glyph we can use here
                            let slice_width = texture.slice_width(&slice);

                            let left = if glyph_idx == 0 { left } else { 0.0 };
                            let right = (slice_width as f32 + left) - cell_width;

                            let bottom = ((texture.coords.height as f32) * glyph.scale + top) -
                                cell_height;

                            vert[V_TOP_LEFT].tex = texture.top_left(&slice);
                            vert[V_TOP_LEFT].adjust = Point::new(left, top);

                            vert[V_TOP_RIGHT].tex = texture.top_right(&slice);
                            vert[V_TOP_RIGHT].adjust = Point::new(right, top);

                            vert[V_BOT_LEFT].tex = texture.bottom_left(&slice);
                            vert[V_BOT_LEFT].adjust = Point::new(left, bottom);

                            vert[V_BOT_RIGHT].tex = texture.bottom_right(&slice);
                            vert[V_BOT_RIGHT].adjust = Point::new(right, bottom);

                            let has_color = if glyph.has_color { 1.0 } else { 0.0 };
                            vert[V_TOP_LEFT].has_color = has_color;
                            vert[V_TOP_RIGHT].has_color = has_color;
                            vert[V_BOT_LEFT].has_color = has_color;
                            vert[V_BOT_RIGHT].has_color = has_color;
                        }
                        &None => {
                            // Whitespace; no texture to render
                            let zero = (0.0, 0.0f32);

                            vert[V_TOP_LEFT].tex = zero;
                            vert[V_TOP_RIGHT].tex = zero;
                            vert[V_BOT_LEFT].tex = zero;
                            vert[V_BOT_RIGHT].tex = zero;

                            vert[V_TOP_LEFT].has_color = 0.0;
                            vert[V_TOP_RIGHT].has_color = 0.0;
                            vert[V_BOT_LEFT].has_color = 0.0;
                            vert[V_BOT_RIGHT].has_color = 0.0;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    pub fn paint(&mut self) -> Result<(), Error> {
        let mut target = self.host.window.draw();
        let res = self.do_paint(&mut target);
        // Ensure that we finish() the target before we let the
        // error bubble up, otherwise we lose the context.
        target.finish().unwrap();
        res?;
        Ok(())
    }

    fn do_paint(&mut self, target: &mut glium::Frame) -> Result<(), Error> {
        let background_color = self.palette.resolve(
            &term::color::ColorAttribute::Background,
        );
        let (r, g, b, a) = background_color.to_linear_tuple_rgba();
        target.clear_color(r, g, b, a);

        let cursor = self.terminal.cursor_pos();
        {
            let dirty_lines = self.terminal.get_dirty_lines();

            for (line_idx, line, selrange) in dirty_lines {
                self.render_screen_line(line_idx, line, selrange, &cursor)?;
            }
        }

        let tex = self.atlas.borrow().texture();

        // Pass 1: Draw backgrounds
        target.draw(
            &*self.glyph_vertex_buffer.borrow(),
            &self.glyph_index_buffer,
            &self.program,
            &uniform! {
                    projection: self.projection.to_column_arrays(),
                    glyph_tex: &*tex,
                    bg_fill: true,
                    underlining: false,
                    underline_tex: &self.underline_tex,
                },
            &glium::DrawParameters {
                blend: glium::Blend::alpha_blending(),
                dithering: false,
                ..Default::default()
            },
        )?;

        // Pass 2: Draw glyphs
        target.draw(
            &*self.glyph_vertex_buffer.borrow(),
            &self.glyph_index_buffer,
            &self.program,
            &uniform! {
                    projection: self.projection.to_column_arrays(),
                    glyph_tex: &*tex,
                    bg_fill: false,
                    underlining: false,
                },
            &glium::DrawParameters {
                blend: glium::Blend::alpha_blending(),
                dithering: false,
                ..Default::default()
            },
        )?;

        // Pass 3: Draw underline/strikethrough
        target.draw(
            &*self.glyph_vertex_buffer.borrow(),
            &self.glyph_index_buffer,
            &self.program,
            &uniform! {
                    projection: self.projection.to_column_arrays(),
                    glyph_tex: &*tex,
                    bg_fill: false,
                    underlining: true,
                },
            &glium::DrawParameters {
                blend: glium::Blend::alpha_blending(),
                dithering: false,
                ..Default::default()
            },
        )?;

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
