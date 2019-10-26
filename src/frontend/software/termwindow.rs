use crate::config::Config;
use crate::config::TextStyle;
use crate::font::{FontConfiguration, FontSystemSelection, GlyphInfo};
use crate::frontend::guicommon::clipboard::SystemClipboard;
use crate::frontend::{front_end, gui_executor};
use crate::keyassignment::{KeyAssignment, KeyMap, SpawnTabDomain};
use crate::mux::renderable::Renderable;
use crate::mux::tab::{Tab, TabId};
use crate::mux::window::WindowId as MuxWindowId;
use crate::mux::Mux;
use ::window::bitmaps::atlas::{Atlas, OutOfTextureSpace, Sprite, SpriteSlice};
use ::window::bitmaps::{Image, ImageTexture, Texture2d, TextureRect};
use ::window::glium::backend::Context as GliumContext;
use ::window::glium::texture::SrgbTexture2d;
use ::window::glium::{uniform, IndexBuffer, Surface, VertexBuffer};
use ::window::*;
use failure::Fallible;
use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;
use term::color::ColorPalette;
use term::{CursorPosition, Line, Underline};
use termwiz::color::RgbColor;

/// Each cell is composed of two triangles built from 4 vertices.
/// The buffer is organized row by row.
const VERTICES_PER_CELL: usize = 4;
const V_TOP_LEFT: usize = 0;
const V_TOP_RIGHT: usize = 1;
const V_BOT_LEFT: usize = 2;
const V_BOT_RIGHT: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct GlyphKey {
    font_idx: usize,
    glyph_pos: u32,
    style: TextStyle,
}

/// Caches a rendered glyph.
/// The image data may be None for whitespace glyphs.
struct CachedGlyph<T: Texture2d> {
    has_color: bool,
    x_offset: f64,
    y_offset: f64,
    bearing_x: f64,
    bearing_y: f64,
    texture: Option<Sprite<T>>,
    scale: f64,
}

struct GlyphCache<T: Texture2d> {
    glyph_cache: HashMap<GlyphKey, Rc<CachedGlyph<T>>>,
    atlas: Atlas<T>,
    fonts: Rc<FontConfiguration>,
    byte_swap: bool,
}

impl GlyphCache<ImageTexture> {
    pub fn new(fonts: &Rc<FontConfiguration>, size: usize) -> Self {
        let surface = Rc::new(ImageTexture::new(size, size));
        let atlas = Atlas::new(&surface).expect("failed to create new texture atlas");

        Self {
            fonts: Rc::clone(fonts),
            glyph_cache: HashMap::new(),
            atlas,
            byte_swap: true,
        }
    }
}

impl GlyphCache<SrgbTexture2d> {
    pub fn new_gl(
        backend: &Rc<GliumContext>,
        fonts: &Rc<FontConfiguration>,
        size: usize,
    ) -> Fallible<Self> {
        let surface = Rc::new(SrgbTexture2d::empty_with_format(
            backend,
            glium::texture::SrgbFormat::U8U8U8U8,
            glium::texture::MipmapsOption::NoMipmap,
            size as u32,
            size as u32,
        )?);
        let atlas = Atlas::new(&surface).expect("failed to create new texture atlas");

        Ok(Self {
            fonts: Rc::clone(fonts),
            glyph_cache: HashMap::new(),
            atlas,
            byte_swap: false,
        })
    }
}

impl<T: Texture2d> GlyphCache<T> {
    /// Resolve a glyph from the cache, rendering the glyph on-demand if
    /// the cache doesn't already hold the desired glyph.
    pub fn cached_glyph(
        &mut self,
        info: &GlyphInfo,
        style: &TextStyle,
    ) -> Fallible<Rc<CachedGlyph<T>>> {
        let key = GlyphKey {
            font_idx: info.font_idx,
            glyph_pos: info.glyph_pos,
            style: style.clone(),
        };

        if let Some(entry) = self.glyph_cache.get(&key) {
            return Ok(Rc::clone(entry));
        }

        let glyph = self.load_glyph(info, style)?;
        self.glyph_cache.insert(key, Rc::clone(&glyph));
        Ok(glyph)
    }

    /// Perform the load and render of a glyph
    #[allow(clippy::float_cmp)]
    fn load_glyph(&mut self, info: &GlyphInfo, style: &TextStyle) -> Fallible<Rc<CachedGlyph<T>>> {
        let (has_color, glyph, cell_width, cell_height) = {
            let font = self.fonts.cached_font(style)?;
            let mut font = font.borrow_mut();
            let metrics = font.get_fallback(0)?.metrics();
            let active_font = font.get_fallback(info.font_idx)?;
            let has_color = active_font.has_color();
            let glyph = active_font.rasterize_glyph(info.glyph_pos)?;
            (has_color, glyph, metrics.cell_width, metrics.cell_height)
        };

        let scale = if (info.x_advance / f64::from(info.num_cells)).floor() > cell_width {
            f64::from(info.num_cells) * (cell_width / info.x_advance)
        } else if glyph.height as f64 > cell_height {
            cell_height / glyph.height as f64
        } else {
            1.0f64
        };
        let glyph = if glyph.width == 0 || glyph.height == 0 {
            // a whitespace glyph
            CachedGlyph {
                has_color,
                texture: None,
                x_offset: info.x_offset * scale,
                y_offset: info.y_offset * scale,
                bearing_x: 0.0,
                bearing_y: 0.0,
                scale,
            }
        } else {
            let raw_im = if self.byte_swap {
                Image::with_rgba32(
                    glyph.width as usize,
                    glyph.height as usize,
                    4 * glyph.width as usize,
                    &glyph.data,
                )
            } else {
                Image::with_bgra32(
                    glyph.width as usize,
                    glyph.height as usize,
                    4 * glyph.width as usize,
                    &glyph.data,
                )
            };

            let bearing_x = glyph.bearing_x * scale;
            let bearing_y = glyph.bearing_y * scale;
            let x_offset = info.x_offset * scale;
            let y_offset = info.y_offset * scale;

            let (scale, raw_im) = if scale != 1.0 {
                (1.0, raw_im.scale_by(scale))
            } else {
                (scale, raw_im)
            };

            let tex = self.atlas.allocate(&raw_im)?;

            CachedGlyph {
                has_color,
                texture: Some(tex),
                x_offset,
                y_offset,
                bearing_x,
                bearing_y,
                scale,
            }
        };

        Ok(Rc::new(glyph))
    }
}

#[derive(Copy, Clone, Default)]
struct Vertex {
    // Physical position of the corner of the character cell
    position: (f32, f32),
    // bearing offset within the cell
    adjust: (f32, f32),
    // glyph texture
    tex: (f32, f32),
    // underline texture
    underline: (f32, f32),
    bg_color: (f32, f32, f32, f32),
    fg_color: (f32, f32, f32, f32),
    // "bool can't be an in in the vertex shader"
    has_color: f32,
}
::window::glium::implement_vertex!(
    Vertex, position, adjust, tex, underline, bg_color, fg_color, has_color
);

/// A helper for updating the 4 vertices that compose a glyph cell
struct Quad<'a> {
    vert: &'a mut [Vertex],
}

impl<'a> Quad<'a> {
    /// Returns a reference to the Quad for the given cell column index
    /// into the set of vertices for a line.
    pub fn for_cell(cell_idx: usize, vertices: &'a mut [Vertex]) -> Self {
        let vert_idx = cell_idx * VERTICES_PER_CELL;
        let vert = &mut vertices[vert_idx..vert_idx + VERTICES_PER_CELL];
        Self { vert }
    }

    /// Assign the texture coordinates
    pub fn set_texture(&mut self, coords: TextureRect) {
        self.vert[V_TOP_LEFT].tex = (coords.min_x(), coords.min_y());
        self.vert[V_TOP_RIGHT].tex = (coords.max_x(), coords.min_y());
        self.vert[V_BOT_LEFT].tex = (coords.min_x(), coords.max_y());
        self.vert[V_BOT_RIGHT].tex = (coords.max_x(), coords.max_y());
    }

    /// Apply bearing adjustment for the glyph texture.
    pub fn set_texture_adjust(&mut self, left: f32, top: f32, right: f32, bottom: f32) {
        self.vert[V_TOP_LEFT].adjust = (left, top);
        self.vert[V_TOP_RIGHT].adjust = (right, top);
        self.vert[V_BOT_LEFT].adjust = (left, bottom);
        self.vert[V_BOT_RIGHT].adjust = (right, bottom);
    }

    /// Set the color glyph "flag"
    pub fn set_has_color(&mut self, has_color: bool) {
        let has_color = if has_color { 1. } else { 0. };
        for v in self.vert.iter_mut() {
            v.has_color = has_color;
        }
    }

    pub fn set_fg_color(&mut self, color: Color) {
        let color = color.to_tuple_rgba();
        for v in self.vert.iter_mut() {
            v.fg_color = color;
        }
    }

    pub fn set_bg_color(&mut self, color: Color) {
        let color = color.to_tuple_rgba();
        for v in self.vert.iter_mut() {
            v.bg_color = color;
        }
    }

    /// Assign the underline texture coordinates for the cell
    pub fn set_underline(&mut self, coords: TextureRect) {
        self.vert[V_TOP_LEFT].underline = (coords.min_x(), coords.min_y());
        self.vert[V_TOP_RIGHT].underline = (coords.max_x(), coords.min_y());
        self.vert[V_BOT_LEFT].underline = (coords.min_x(), coords.max_y());
        self.vert[V_BOT_RIGHT].underline = (coords.max_x(), coords.max_y());
    }
}

#[derive(Copy, Clone)]
struct RenderMetrics {
    descender: f64,
    descender_row: isize,
    descender_plus_two: isize,
    underline_height: isize,
    strike_row: isize,
    cell_size: Size,
}

impl RenderMetrics {
    fn new(fonts: &Rc<FontConfiguration>) -> Self {
        let metrics = fonts
            .default_font_metrics()
            .expect("failed to get font metrics!?");

        let (cell_height, cell_width) = (
            metrics.cell_height.ceil() as usize,
            metrics.cell_width.ceil() as usize,
        );

        let underline_height = metrics.underline_thickness.round() as isize;

        let descender_row =
            (cell_height as f64 + metrics.descender - metrics.underline_position) as isize;
        let descender_plus_two =
            (2 * underline_height + descender_row).min(cell_height as isize - 1);
        let strike_row = descender_row / 2;

        Self {
            descender: metrics.descender,
            descender_row,
            descender_plus_two,
            strike_row,
            cell_size: Size::new(cell_width as isize, cell_height as isize),
            underline_height,
        }
    }
}

struct SoftwareRenderState {
    glyph_cache: RefCell<GlyphCache<ImageTexture>>,
    util_sprites: UtilSprites<ImageTexture>,
}

impl SoftwareRenderState {
    pub fn new(
        fonts: &Rc<FontConfiguration>,
        metrics: &RenderMetrics,
        size: usize,
    ) -> Fallible<Self> {
        let glyph_cache = RefCell::new(GlyphCache::new(fonts, size));
        let util_sprites = UtilSprites::new(&mut glyph_cache.borrow_mut(), metrics)?;
        Ok(Self {
            glyph_cache,
            util_sprites,
        })
    }
}

struct UtilSprites<T: Texture2d> {
    white_space: Sprite<T>,
    single_underline: Sprite<T>,
    double_underline: Sprite<T>,
    strike_through: Sprite<T>,
    single_and_strike: Sprite<T>,
    double_and_strike: Sprite<T>,
}

impl<T: Texture2d> UtilSprites<T> {
    fn new(
        glyph_cache: &mut GlyphCache<T>,
        metrics: &RenderMetrics,
    ) -> Result<Self, OutOfTextureSpace> {
        let mut buffer = Image::new(
            metrics.cell_size.width as usize,
            metrics.cell_size.height as usize,
        );

        let black = ::window::color::Color::rgba(0, 0, 0, 0);
        let white = ::window::color::Color::rgb(0xff, 0xff, 0xff);

        let cell_rect = Rect::new(Point::new(0, 0), metrics.cell_size);

        buffer.clear_rect(cell_rect, black);
        let white_space = glyph_cache.atlas.allocate(&buffer)?;

        let draw_single = |buffer: &mut Image| {
            for row in 0..metrics.underline_height {
                buffer.draw_line(
                    Point::new(
                        cell_rect.origin.x,
                        cell_rect.origin.y + metrics.descender_row + row,
                    ),
                    Point::new(
                        cell_rect.origin.x + metrics.cell_size.width,
                        cell_rect.origin.y + metrics.descender_row + row,
                    ),
                    white,
                    Operator::Source,
                );
            }
        };

        let draw_double = |buffer: &mut Image| {
            for row in 0..metrics.underline_height {
                buffer.draw_line(
                    Point::new(
                        cell_rect.origin.x,
                        cell_rect.origin.y + metrics.descender_row + row,
                    ),
                    Point::new(
                        cell_rect.origin.x + metrics.cell_size.width,
                        cell_rect.origin.y + metrics.descender_row + row,
                    ),
                    white,
                    Operator::Source,
                );
                buffer.draw_line(
                    Point::new(
                        cell_rect.origin.x,
                        cell_rect.origin.y + metrics.descender_plus_two + row,
                    ),
                    Point::new(
                        cell_rect.origin.x + metrics.cell_size.width,
                        cell_rect.origin.y + metrics.descender_plus_two + row,
                    ),
                    white,
                    Operator::Source,
                );
            }
        };

        let draw_strike = |buffer: &mut Image| {
            for row in 0..metrics.underline_height {
                buffer.draw_line(
                    Point::new(
                        cell_rect.origin.x,
                        cell_rect.origin.y + metrics.strike_row + row,
                    ),
                    Point::new(
                        cell_rect.origin.x + metrics.cell_size.width,
                        cell_rect.origin.y + metrics.strike_row + row,
                    ),
                    white,
                    Operator::Source,
                );
            }
        };

        buffer.clear_rect(cell_rect, black);
        draw_single(&mut buffer);
        let single_underline = glyph_cache.atlas.allocate(&buffer)?;

        buffer.clear_rect(cell_rect, black);
        draw_double(&mut buffer);
        let double_underline = glyph_cache.atlas.allocate(&buffer)?;

        buffer.clear_rect(cell_rect, black);
        draw_strike(&mut buffer);
        let strike_through = glyph_cache.atlas.allocate(&buffer)?;

        buffer.clear_rect(cell_rect, black);
        draw_single(&mut buffer);
        draw_strike(&mut buffer);
        let single_and_strike = glyph_cache.atlas.allocate(&buffer)?;

        buffer.clear_rect(cell_rect, black);
        draw_double(&mut buffer);
        draw_strike(&mut buffer);
        let double_and_strike = glyph_cache.atlas.allocate(&buffer)?;

        Ok(Self {
            white_space,
            single_underline,
            double_underline,
            strike_through,
            single_and_strike,
            double_and_strike,
        })
    }

    /// Figure out what we're going to draw for the underline.
    /// If the current cell is part of the current URL highlight
    /// then we want to show the underline.
    pub fn select_sprite(
        &self,
        is_highlited_hyperlink: bool,
        is_strike_through: bool,
        underline: Underline,
    ) -> &Sprite<T> {
        match (is_highlited_hyperlink, is_strike_through, underline) {
            (true, false, Underline::None) => &self.single_underline,
            (true, false, Underline::Single) => &self.double_underline,
            (true, false, Underline::Double) => &self.single_underline,
            (true, true, Underline::None) => &self.strike_through,
            (true, true, Underline::Single) => &self.single_and_strike,
            (true, true, Underline::Double) => &self.double_and_strike,
            (false, false, Underline::None) => &self.white_space,
            (false, false, Underline::Single) => &self.single_underline,
            (false, false, Underline::Double) => &self.double_underline,
            (false, true, Underline::None) => &self.strike_through,
            (false, true, Underline::Single) => &self.single_and_strike,
            (false, true, Underline::Double) => &self.double_and_strike,
        }
    }
}

struct OpenGLRenderState {
    context: Rc<GliumContext>,
    glyph_cache: RefCell<GlyphCache<SrgbTexture2d>>,
    util_sprites: UtilSprites<SrgbTexture2d>,
    program: glium::Program,
    glyph_vertex_buffer: RefCell<VertexBuffer<Vertex>>,
    glyph_index_buffer: IndexBuffer<u32>,
}

impl OpenGLRenderState {
    pub fn new(
        context: Rc<GliumContext>,
        fonts: &Rc<FontConfiguration>,
        metrics: &RenderMetrics,
        size: usize,
        pixel_width: usize,
        pixel_height: usize,
    ) -> Fallible<Self> {
        let glyph_cache = RefCell::new(GlyphCache::new_gl(&context, fonts, size)?);
        let util_sprites = UtilSprites::new(&mut *glyph_cache.borrow_mut(), metrics)?;

        let mut errors = vec![];
        let mut program = None;
        for version in &["330", "300 es"] {
            let source = glium::program::ProgramCreationInput::SourceCode {
                vertex_shader: &Self::vertex_shader(version),
                fragment_shader: &Self::fragment_shader(version),
                outputs_srgb: true,
                tessellation_control_shader: None,
                tessellation_evaluation_shader: None,
                transform_feedback_varyings: None,
                uses_point_size: false,
                geometry_shader: None,
            };
            log::error!("compiling a prog with version {}", version);
            match glium::Program::new(&context, source) {
                Ok(prog) => {
                    program = Some(prog);
                    break;
                }
                Err(err) => errors.push(err.to_string()),
            };
        }

        let program = program.ok_or_else(|| {
            failure::format_err!("Failed to compile shaders: {}", errors.join("\n"))
        })?;

        let (glyph_vertex_buffer, glyph_index_buffer) =
            Self::compute_vertices(&context, metrics, pixel_width as f32, pixel_height as f32)?;

        Ok(Self {
            context,
            glyph_cache,
            util_sprites,
            program,
            glyph_vertex_buffer: RefCell::new(glyph_vertex_buffer),
            glyph_index_buffer,
        })
    }

    pub fn advise_of_window_size_change(
        &mut self,
        metrics: &RenderMetrics,
        pixel_width: usize,
        pixel_height: usize,
    ) -> Fallible<()> {
        let (glyph_vertex_buffer, glyph_index_buffer) = Self::compute_vertices(
            &self.context,
            metrics,
            pixel_width as f32,
            pixel_height as f32,
        )?;

        *self.glyph_vertex_buffer.borrow_mut() = glyph_vertex_buffer;
        self.glyph_index_buffer = glyph_index_buffer;
        Ok(())
    }

    fn vertex_shader(version: &str) -> String {
        format!("#version {}\n{}", version, include_str!("vertex.glsl"))
    }

    fn fragment_shader(version: &str) -> String {
        format!("#version {}\n{}", version, include_str!("fragment.glsl"))
    }

    /// Compute a vertex buffer to hold the quads that comprise the visible
    /// portion of the screen.   We recreate this when the screen is resized.
    /// The idea is that we want to minimize and heavy lifting and computation
    /// and instead just poke some attributes into the offset that corresponds
    /// to a changed cell when we need to repaint the screen, and then just
    /// let the GPU figure out the rest.
    fn compute_vertices(
        context: &Rc<GliumContext>,
        metrics: &RenderMetrics,
        width: f32,
        height: f32,
    ) -> Fallible<(VertexBuffer<Vertex>, IndexBuffer<u32>)> {
        let cell_width = metrics.cell_size.width as f32;
        let cell_height = metrics.cell_size.height as f32;
        let mut verts = Vec::new();
        let mut indices = Vec::new();

        let num_cols = (width as usize + 1) / cell_width as usize;
        let num_rows = (height as usize + 1) / cell_height as usize;

        for y in 0..num_rows {
            for x in 0..num_cols {
                let y_pos = (height / -2.0) + (y as f32 * cell_height);
                let x_pos = (width / -2.0) + (x as f32 * cell_width);
                // Remember starting index for this position
                let idx = verts.len() as u32;
                verts.push(Vertex {
                    // Top left
                    position: (x_pos, y_pos),
                    ..Default::default()
                });
                verts.push(Vertex {
                    // Top Right
                    position: (x_pos + cell_width, y_pos),
                    ..Default::default()
                });
                verts.push(Vertex {
                    // Bottom Left
                    position: (x_pos, y_pos + cell_height),
                    ..Default::default()
                });
                verts.push(Vertex {
                    // Bottom Right
                    position: (x_pos + cell_width, y_pos + cell_height),
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
            VertexBuffer::dynamic(context, &verts)?,
            IndexBuffer::new(
                context,
                glium::index::PrimitiveType::TrianglesList,
                &indices,
            )?,
        ))
    }
}

#[allow(clippy::large_enum_variant)]
enum RenderState {
    Software(SoftwareRenderState),
    GL(OpenGLRenderState),
}

impl RenderState {
    pub fn recreate_texture_atlas(
        &mut self,
        fonts: &Rc<FontConfiguration>,
        metrics: &RenderMetrics,
        size: Option<usize>,
    ) -> Fallible<()> {
        match self {
            RenderState::Software(software) => {
                let size = size.unwrap_or_else(|| software.glyph_cache.borrow().atlas.size());
                let mut glyph_cache = GlyphCache::new(fonts, size);
                software.util_sprites = UtilSprites::new(&mut glyph_cache, metrics)?;
                *software.glyph_cache.borrow_mut() = glyph_cache;
            }
            RenderState::GL(gl) => {
                let size = size.unwrap_or_else(|| gl.glyph_cache.borrow().atlas.size());
                let mut glyph_cache = GlyphCache::new_gl(&gl.context, fonts, size)?;
                gl.util_sprites = UtilSprites::new(&mut glyph_cache, metrics)?;
                *gl.glyph_cache.borrow_mut() = glyph_cache;
            }
        };
        Ok(())
    }

    pub fn advise_of_window_size_change(
        &mut self,
        metrics: &RenderMetrics,
        pixel_width: usize,
        pixel_height: usize,
    ) -> Fallible<()> {
        if let RenderState::GL(gl) = self {
            gl.advise_of_window_size_change(metrics, pixel_width, pixel_height)?;
        }
        Ok(())
    }

    pub fn cached_software_glyph(
        &self,
        info: &GlyphInfo,
        style: &TextStyle,
    ) -> Fallible<Rc<CachedGlyph<ImageTexture>>> {
        if let RenderState::Software(software) = self {
            software.glyph_cache.borrow_mut().cached_glyph(info, style)
        } else {
            failure::bail!("attempted to call cached_software_glyph when in gl mode")
        }
    }

    pub fn software(&self) -> &SoftwareRenderState {
        match self {
            RenderState::Software(software) => software,
            _ => panic!("only valid for software render mode"),
        }
    }

    pub fn opengl(&self) -> &OpenGLRenderState {
        match self {
            RenderState::GL(gl) => gl,
            _ => panic!("only valid for opengl render mode"),
        }
    }
}

pub struct TermWindow {
    window: Option<Window>,
    fonts: Rc<FontConfiguration>,
    _config: Arc<Config>,
    dimensions: Dimensions,
    mux_window_id: MuxWindowId,
    render_metrics: RenderMetrics,
    render_state: RenderState,
    clipboard: Arc<dyn term::Clipboard>,
    keys: KeyMap,
}

struct Host<'a> {
    writer: &'a mut dyn std::io::Write,
    context: &'a dyn WindowOps,
    clipboard: &'a Arc<dyn term::Clipboard>,
}

impl<'a> term::TerminalHost for Host<'a> {
    fn writer(&mut self) -> &mut dyn std::io::Write {
        self.writer
    }

    fn get_clipboard(&mut self) -> Fallible<Arc<dyn term::Clipboard>> {
        Ok(Arc::clone(self.clipboard))
    }

    fn set_title(&mut self, title: &str) {
        self.context.set_title(title);
    }

    fn click_link(&mut self, link: &Arc<term::cell::Hyperlink>) {
        log::error!("clicking {}", link.uri());
        if let Err(err) = open::that(link.uri()) {
            log::error!("failed to open {}: {:?}", link.uri(), err);
        }
    }
}

impl WindowCallbacks for TermWindow {
    fn created(&mut self, window: &Window) {
        self.window.replace(window.clone());
    }

    fn can_close(&mut self) -> bool {
        // can_close triggers the current tab to be closed.
        // If we have no tabs left then we can close the whole window.
        // If we're in a weird state, then we allow the window to close too.
        let mux = Mux::get().unwrap();
        let tab = match mux.get_active_tab_for_window(self.mux_window_id) {
            Some(tab) => tab,
            None => return true,
        };
        mux.remove_tab(tab.tab_id());
        if let Some(mut win) = mux.get_window_mut(self.mux_window_id) {
            win.remove_by_id(tab.tab_id());
            return win.is_empty();
        };
        true
    }

    fn as_any(&mut self) -> &mut dyn Any {
        self
    }

    fn mouse_event(&mut self, event: &MouseEvent, context: &dyn WindowOps) {
        let mux = Mux::get().unwrap();
        let tab = match mux.get_active_tab_for_window(self.mux_window_id) {
            Some(tab) => tab,
            None => return,
        };

        use ::term::input::MouseButton as TMB;
        use ::term::input::MouseEventKind as TMEK;
        use ::window::MouseButtons as WMB;
        use ::window::MouseEventKind as WMEK;
        tab.mouse_event(
            term::MouseEvent {
                kind: match event.kind {
                    WMEK::Move => TMEK::Move,
                    WMEK::VertWheel(_)
                    | WMEK::HorzWheel(_)
                    | WMEK::DoubleClick(_)
                    | WMEK::Press(_) => TMEK::Press,
                    WMEK::Release(_) => TMEK::Release,
                },
                button: match event.kind {
                    WMEK::Release(ref press)
                    | WMEK::Press(ref press)
                    | WMEK::DoubleClick(ref press) => match press {
                        MousePress::Left => TMB::Left,
                        MousePress::Middle => TMB::Middle,
                        MousePress::Right => TMB::Right,
                    },
                    WMEK::Move => {
                        if event.mouse_buttons == WMB::LEFT {
                            TMB::Left
                        } else if event.mouse_buttons == WMB::RIGHT {
                            TMB::Right
                        } else if event.mouse_buttons == WMB::MIDDLE {
                            TMB::Middle
                        } else {
                            TMB::None
                        }
                    }
                    WMEK::VertWheel(amount) => {
                        if amount > 0 {
                            TMB::WheelUp(amount as usize)
                        } else {
                            TMB::WheelDown((-amount) as usize)
                        }
                    }
                    WMEK::HorzWheel(_) => TMB::None,
                },
                x: (event.x as isize / self.render_metrics.cell_size.width) as usize,
                y: (event.y as isize / self.render_metrics.cell_size.height) as i64,
                modifiers: window_mods_to_termwiz_mods(event.modifiers),
            },
            &mut Host {
                writer: &mut *tab.writer(),
                context,
                clipboard: &self.clipboard,
            },
        )
        .ok();

        match event.kind {
            WMEK::Move => {}
            _ => context.invalidate(),
        }

        // When hovering over a hyperlink, show an appropriate
        // mouse cursor to give the cue that it is clickable
        context.set_cursor(Some(if tab.renderer().current_highlight().is_some() {
            MouseCursor::Hand
        } else {
            MouseCursor::Text
        }));
    }

    fn resize(&mut self, dimensions: Dimensions) {
        if dimensions.pixel_width == 0 || dimensions.pixel_height == 0 {
            // on windows, this can happen when minimizing the window.
            // NOP!
            return;
        }
        self.scaling_changed(dimensions, self.fonts.get_font_scale());
    }

    fn key_event(&mut self, key: &KeyEvent, _context: &dyn WindowOps) -> bool {
        if !key.key_is_down {
            return false;
        }

        let mux = Mux::get().unwrap();
        if let Some(tab) = mux.get_active_tab_for_window(self.mux_window_id) {
            let modifiers = window_mods_to_termwiz_mods(key.modifiers);

            use ::termwiz::input::KeyCode as KC;
            use ::window::KeyCode as WK;

            let key_down = match key.key {
                WK::Char(c) => Some(KC::Char(c)),
                WK::Composed(ref s) => {
                    tab.writer().write_all(s.as_bytes()).ok();
                    return true;
                }
                WK::Function(f) => Some(KC::Function(f)),
                WK::LeftArrow => Some(KC::LeftArrow),
                WK::RightArrow => Some(KC::RightArrow),
                WK::UpArrow => Some(KC::UpArrow),
                WK::DownArrow => Some(KC::DownArrow),
                WK::Home => Some(KC::Home),
                WK::End => Some(KC::End),
                WK::PageUp => Some(KC::PageUp),
                WK::PageDown => Some(KC::PageDown),
                // TODO: more keys (eg: numpad!)
                _ => None,
            };

            if let Some(key) = key_down {
                if let Some(assignment) = self.keys.lookup(key, modifiers) {
                    self.perform_key_assignment(&tab, &assignment).ok();
                    return true;
                } else if tab.key_down(key, modifiers).is_ok() {
                    return true;
                }
            }
        }

        false
    }

    fn paint(&mut self, ctx: &mut dyn PaintContext) {
        let mux = Mux::get().unwrap();
        let tab = match mux.get_active_tab_for_window(self.mux_window_id) {
            Some(tab) => tab,
            None => {
                ctx.clear(Color::rgb(0, 0, 0));
                return;
            }
        };
        let start = std::time::Instant::now();
        if let Err(err) = self.paint_tab(&tab, ctx) {
            if let Some(&OutOfTextureSpace { size }) = err.downcast_ref::<OutOfTextureSpace>() {
                log::error!("out of texture space, allocating {}", size);
                if let Err(err) = self.recreate_texture_atlas(Some(size)) {
                    log::error!("failed recreate atlas with size {}: {}", size, err);
                    // Failed to increase the size.
                    // This might happen if a lot of images have been displayed in the
                    // terminal over time and we've hit a texture size limit.
                    // Let's just try recreating at the current size.
                    self.recreate_texture_atlas(None)
                        .expect("OutOfTextureSpace and failed to recreate atlas");
                }
                tab.renderer().make_all_lines_dirty();
                // Recursively initiate a new paint
                return self.paint(ctx);
            }
            log::error!("paint failed: {}", err);
        }
        log::debug!("paint_tab elapsed={:?}", start.elapsed());
        self.update_title();
    }

    fn paint_opengl(&mut self, frame: &mut glium::Frame) {
        let mux = Mux::get().unwrap();
        let tab = match mux.get_active_tab_for_window(self.mux_window_id) {
            Some(tab) => tab,
            None => {
                frame.clear_color(0., 0., 0., 1.);
                return;
            }
        };
        let start = std::time::Instant::now();
        if let Err(err) = self.paint_tab_opengl(&tab, frame) {
            if let Some(&OutOfTextureSpace { size }) = err.downcast_ref::<OutOfTextureSpace>() {
                log::error!("out of texture space, allocating {}", size);
                if let Err(err) = self.recreate_texture_atlas(Some(size)) {
                    log::error!("failed recreate atlas with size {}: {}", size, err);
                    // Failed to increase the size.
                    // This might happen if a lot of images have been displayed in the
                    // terminal over time and we've hit a texture size limit.
                    // Let's just try recreating at the current size.
                    self.recreate_texture_atlas(None)
                        .expect("OutOfTextureSpace and failed to recreate atlas");
                }
                tab.renderer().make_all_lines_dirty();
                // Recursively initiate a new paint
                return self.paint_opengl(frame);
            }
            log::error!("paint_tab_opengl failed: {}", err);
        }
        log::debug!("paint_tab_opengl elapsed={:?}", start.elapsed());
        self.update_title();
    }
}

impl TermWindow {
    pub fn new_window(
        config: &Arc<Config>,
        fontconfig: &Rc<FontConfiguration>,
        tab: &Rc<dyn Tab>,
        mux_window_id: MuxWindowId,
    ) -> Fallible<()> {
        log::error!(
            "TermWindow::new_window called with mux_window_id {}",
            mux_window_id
        );
        let (physical_rows, physical_cols) = tab.renderer().physical_dimensions();

        let render_metrics = RenderMetrics::new(fontconfig);

        let width = render_metrics.cell_size.width as usize * physical_cols;
        let height = render_metrics.cell_size.height as usize * physical_rows;

        const ATLAS_SIZE: usize = 4096;
        let render_state = RenderState::Software(SoftwareRenderState::new(
            fontconfig,
            &render_metrics,
            ATLAS_SIZE,
        )?);

        let window = Window::new_window(
            "wezterm",
            "wezterm",
            width,
            height,
            Box::new(Self {
                window: None,
                mux_window_id,
                _config: Arc::clone(config),
                fonts: Rc::clone(fontconfig),
                render_metrics,
                dimensions: Dimensions {
                    pixel_width: width,
                    pixel_height: height,
                    // This is the default dpi; we'll get a resize
                    // event to inform us of the true dpi if it is
                    // different from this value
                    dpi: 96,
                },
                render_state,
                clipboard: Arc::new(SystemClipboard::new()),
                keys: KeyMap::new(),
            }),
        )?;

        let cloned_window = window.clone();

        Connection::get().unwrap().schedule_timer(
            std::time::Duration::from_millis(35),
            move || {
                let mux = Mux::get().unwrap();
                if let Some(tab) = mux.get_active_tab_for_window(mux_window_id) {
                    if tab.renderer().has_dirty_lines() {
                        cloned_window.invalidate();
                    }
                } else {
                    cloned_window.close();
                }
            },
        );

        window.show();

        if super::is_opengl_enabled() {
            window.enable_opengl(|any, _window, maybe_ctx| {
                let mut termwindow = any.downcast_mut::<TermWindow>().expect("to be TermWindow");

                match maybe_ctx {
                    Ok(ctx) => {
                        match OpenGLRenderState::new(
                            ctx,
                            &termwindow.fonts,
                            &termwindow.render_metrics,
                            ATLAS_SIZE,
                            termwindow.dimensions.pixel_width,
                            termwindow.dimensions.pixel_height,
                        ) {
                            Ok(gl) => {
                                log::error!(
                                    "OpenGL initialized! {} {}",
                                    gl.context.get_opengl_renderer_string(),
                                    gl.context.get_opengl_version_string()
                                );
                                termwindow.render_state = RenderState::GL(gl);
                            }
                            Err(err) => {
                                log::error!("OpenGL init failed: {}", err);
                            }
                        }
                    }
                    Err(err) => log::error!("OpenGL init failed: {}", err),
                }
            });
        }

        Ok(())
    }

    fn recreate_texture_atlas(&mut self, size: Option<usize>) -> Fallible<()> {
        self.render_state
            .recreate_texture_atlas(&self.fonts, &self.render_metrics, size)
    }

    fn update_title(&mut self) {
        let mux = Mux::get().unwrap();
        let window = match mux.get_window(self.mux_window_id) {
            Some(window) => window,
            _ => return,
        };
        let num_tabs = window.len();

        if num_tabs == 0 {
            return;
        }
        let tab_no = window.get_active_idx();

        let title = match window.get_active() {
            Some(tab) => tab.get_title(),
            None => return,
        };

        drop(window);

        if let Some(window) = self.window.as_ref() {
            if num_tabs == 1 {
                window.set_title(&title);
            } else {
                window.set_title(&format!("[{}/{}] {}", tab_no + 1, num_tabs, title));
            }
        }
    }

    fn activate_tab(&mut self, tab_idx: usize) -> Fallible<()> {
        let mux = Mux::get().unwrap();
        let mut window = mux
            .get_window_mut(self.mux_window_id)
            .ok_or_else(|| failure::format_err!("no such window"))?;

        let max = window.len();
        if tab_idx < max {
            window.set_active(tab_idx);

            drop(window);
            self.update_title();
        }
        Ok(())
    }

    fn activate_tab_relative(&mut self, delta: isize) -> Fallible<()> {
        let mux = Mux::get().unwrap();
        let window = mux
            .get_window(self.mux_window_id)
            .ok_or_else(|| failure::format_err!("no such window"))?;

        let max = window.len();
        failure::ensure!(max > 0, "no more tabs");

        let active = window.get_active_idx() as isize;
        let tab = active + delta;
        let tab = if tab < 0 { max as isize + tab } else { tab };
        drop(window);
        self.activate_tab(tab as usize % max)
    }

    fn spawn_tab(&mut self, domain: &SpawnTabDomain) -> Fallible<TabId> {
        let rows = (self.dimensions.pixel_height as usize + 1)
            / self.render_metrics.cell_size.height as usize;
        let cols = (self.dimensions.pixel_width as usize + 1)
            / self.render_metrics.cell_size.width as usize;

        let size = portable_pty::PtySize {
            rows: rows as u16,
            cols: cols as u16,
            pixel_width: self.dimensions.pixel_width as u16,
            pixel_height: self.dimensions.pixel_height as u16,
        };

        let mux = Mux::get().unwrap();

        let domain = match domain {
            SpawnTabDomain::DefaultDomain => mux.default_domain().clone(),
            SpawnTabDomain::CurrentTabDomain => {
                let tab = match mux.get_active_tab_for_window(self.mux_window_id) {
                    Some(tab) => tab,
                    None => failure::bail!("window has no tabs?"),
                };
                mux.get_domain(tab.domain_id()).ok_or_else(|| {
                    failure::format_err!("current tab has unresolvable domain id!?")
                })?
            }
            SpawnTabDomain::Domain(id) => mux.get_domain(*id).ok_or_else(|| {
                failure::format_err!("spawn_tab called with unresolvable domain id!?")
            })?,
            SpawnTabDomain::DomainName(name) => mux.get_domain_by_name(&name).ok_or_else(|| {
                failure::format_err!("spawn_tab called with unresolvable domain name {}", name)
            })?,
        };
        let tab = domain.spawn(size, None, self.mux_window_id)?;
        let tab_id = tab.tab_id();

        let len = {
            let window = mux
                .get_window(self.mux_window_id)
                .ok_or_else(|| failure::format_err!("no such window!?"))?;
            window.len()
        };
        self.activate_tab(len - 1)?;
        Ok(tab_id)
    }

    fn perform_key_assignment(
        &mut self,
        tab: &Rc<dyn Tab>,
        assignment: &KeyAssignment,
    ) -> Fallible<()> {
        use KeyAssignment::*;
        match assignment {
            SpawnTab(spawn_where) => {
                self.spawn_tab(spawn_where)?;
            }
            SpawnWindow => {
                self.spawn_new_window();
            }
            ToggleFullScreen => {
                // self.toggle_full_screen(),
            }
            Copy => {
                // Nominally copy, but that is implicit, so NOP
            }
            Paste => {
                tab.trickle_paste(self.clipboard.get_contents()?)?;
            }
            ActivateTabRelative(n) => {
                self.activate_tab_relative(*n)?;
            }
            DecreaseFontSize => self.decrease_font_size(),
            IncreaseFontSize => self.increase_font_size(),
            ResetFontSize => self.reset_font_size(),
            ActivateTab(n) => {
                self.activate_tab(*n)?;
            }
            SendString(s) => tab.writer().write_all(s.as_bytes())?,
            Hide => {
                if let Some(w) = self.window.as_ref() {
                    w.hide();
                }
            }
            Show => {
                if let Some(w) = self.window.as_ref() {
                    w.show();
                }
            }
            CloseCurrentTab => self.close_current_tab(),
            Nop => {}
        };
        Ok(())
    }

    pub fn spawn_new_window(&mut self) {
        promise::Future::with_executor(gui_executor().unwrap(), move || {
            let mux = Mux::get().unwrap();
            let fonts = Rc::new(FontConfiguration::new(
                Arc::clone(mux.config()),
                FontSystemSelection::get_default(),
            ));
            let window_id = mux.new_empty_window();
            let tab =
                mux.default_domain()
                    .spawn(portable_pty::PtySize::default(), None, window_id)?;
            let front_end = front_end().expect("to be called on gui thread");
            front_end.spawn_new_window(mux.config(), &fonts, &tab, window_id)?;
            Ok(())
        });
    }

    #[allow(clippy::float_cmp)]
    fn scaling_changed(&mut self, dimensions: Dimensions, font_scale: f64) {
        let mux = Mux::get().unwrap();
        if let Some(window) = mux.get_window(self.mux_window_id) {
            if dimensions.dpi != self.dimensions.dpi || font_scale != self.fonts.get_font_scale() {
                self.fonts
                    .change_scaling(font_scale, dimensions.dpi as f64 / 96.);
                self.render_metrics = RenderMetrics::new(&self.fonts);

                self.recreate_texture_atlas(None)
                    .expect("failed to recreate atlas");
            }

            self.dimensions = dimensions;

            self.render_state
                .advise_of_window_size_change(
                    &self.render_metrics,
                    dimensions.pixel_width,
                    dimensions.pixel_height,
                )
                .expect("failed to advise of resize");

            let size = portable_pty::PtySize {
                rows: dimensions.pixel_height as u16 / self.render_metrics.cell_size.height as u16,
                cols: dimensions.pixel_width as u16 / self.render_metrics.cell_size.width as u16,
                pixel_height: dimensions.pixel_height as u16,
                pixel_width: dimensions.pixel_width as u16,
            };
            for tab in window.iter() {
                tab.resize(size).ok();
            }
        };
    }

    fn decrease_font_size(&mut self) {
        self.scaling_changed(self.dimensions, self.fonts.get_font_scale() * 0.9);
    }
    fn increase_font_size(&mut self) {
        self.scaling_changed(self.dimensions, self.fonts.get_font_scale() * 1.1);
    }
    fn reset_font_size(&mut self) {
        self.scaling_changed(self.dimensions, 1.);
    }

    fn close_current_tab(&mut self) {
        let mux = Mux::get().unwrap();
        let tab = match mux.get_active_tab_for_window(self.mux_window_id) {
            Some(tab) => tab,
            None => return,
        };
        mux.remove_tab(tab.tab_id());
        if let Some(mut win) = mux.get_window_mut(self.mux_window_id) {
            win.remove_by_id(tab.tab_id());
        }
        self.activate_tab_relative(0).ok();
    }

    fn paint_tab(&mut self, tab: &Rc<dyn Tab>, ctx: &mut dyn PaintContext) -> Fallible<()> {
        let palette = tab.palette();

        let mut term = tab.renderer();
        let cursor = term.get_cursor_position();

        {
            let dirty_lines = term.get_dirty_lines();

            for (line_idx, line, selrange) in dirty_lines {
                self.render_screen_line(ctx, line_idx, &line, selrange, &cursor, &*term, &palette)?;
            }
        }

        term.clean_dirty_lines();

        // Fill any marginal area below the last row
        let (num_rows, _num_cols) = term.physical_dimensions();
        let pixel_height_of_cells = num_rows * self.render_metrics.cell_size.height as usize;
        ctx.clear_rect(
            Rect::new(
                Point::new(0, pixel_height_of_cells as isize),
                Size::new(
                    self.dimensions.pixel_width as isize,
                    (self.dimensions.pixel_height - pixel_height_of_cells) as isize,
                ),
            ),
            rgbcolor_to_window_color(palette.background),
        );
        Ok(())
    }

    fn paint_tab_opengl(&mut self, tab: &Rc<dyn Tab>, frame: &mut glium::Frame) -> Fallible<()> {
        let palette = tab.palette();

        let background_color = palette.resolve_bg(term::color::ColorAttribute::Default);
        let (r, g, b, a) = background_color.to_tuple_rgba();
        frame.clear_color(r, g, b, a);

        let mut term = tab.renderer();
        let cursor = term.get_cursor_position();

        {
            let dirty_lines = term.get_dirty_lines();

            for (line_idx, line, selrange) in dirty_lines {
                self.render_screen_line_opengl(
                    line_idx, &line, selrange, &cursor, &*term, &palette,
                )?;
            }
        }

        let gl_state = self.render_state.opengl();
        let tex = gl_state.glyph_cache.borrow().atlas.texture();
        let projection = euclid::Transform3D::<f32, f32, f32>::ortho(
            -(self.dimensions.pixel_width as f32) / 2.0,
            self.dimensions.pixel_width as f32 / 2.0,
            self.dimensions.pixel_height as f32 / 2.0,
            -(self.dimensions.pixel_height as f32) / 2.0,
            -1.0,
            1.0,
        )
        .to_column_arrays();

        // Pass 1: Draw backgrounds, strikethrough and underline
        frame.draw(
            &*gl_state.glyph_vertex_buffer.borrow(),
            &gl_state.glyph_index_buffer,
            &gl_state.program,
            &uniform! {
                projection: projection,
                glyph_tex: &*tex,
                bg_and_line_layer: true,
            },
            &glium::DrawParameters {
                blend: glium::Blend::alpha_blending(),
                ..Default::default()
            },
        )?;

        // Pass 2: Draw glyphs
        frame.draw(
            &*gl_state.glyph_vertex_buffer.borrow(),
            &gl_state.glyph_index_buffer,
            &gl_state.program,
            &uniform! {
                projection: projection,
                glyph_tex: &*tex,
                bg_and_line_layer: false,
            },
            &glium::DrawParameters {
                blend: glium::Blend::alpha_blending(),
                ..Default::default()
            },
        )?;

        term.clean_dirty_lines();

        Ok(())
    }

    /// "Render" a line of the terminal screen into the vertex buffer.
    /// This is nominally a matter of setting the fg/bg color and the
    /// texture coordinates for a given glyph.  There's a little bit
    /// of extra complexity to deal with multi-cell glyphs.
    fn render_screen_line_opengl(
        &self,
        line_idx: usize,
        line: &Line,
        selection: Range<usize>,
        cursor: &CursorPosition,
        terminal: &dyn Renderable,
        palette: &ColorPalette,
    ) -> Fallible<()> {
        let gl_state = self.render_state.opengl();

        let (_num_rows, num_cols) = terminal.physical_dimensions();
        let mut vb = gl_state.glyph_vertex_buffer.borrow_mut();
        let mut vertices = {
            let per_line = num_cols * VERTICES_PER_CELL;
            let start_pos = line_idx * per_line;
            vb.slice_mut(start_pos..start_pos + per_line)
                .ok_or_else(|| failure::err_msg("we're confused about the screen size"))?
                .map()
        };

        let current_highlight = terminal.current_highlight();

        // Break the line into clusters of cells with the same attributes
        let cell_clusters = line.cluster();
        let mut last_cell_idx = 0;
        for cluster in cell_clusters {
            let attrs = &cluster.attrs;
            let is_highlited_hyperlink = match (&attrs.hyperlink, &current_highlight) {
                (&Some(ref this), &Some(ref highlight)) => this == highlight,
                _ => false,
            };
            let style = self.fonts.match_style(attrs);

            let bg_color = palette.resolve_bg(attrs.background);
            let fg_color = match attrs.foreground {
                term::color::ColorAttribute::Default => {
                    if let Some(fg) = style.foreground {
                        fg
                    } else {
                        palette.resolve_fg(attrs.foreground)
                    }
                }
                term::color::ColorAttribute::PaletteIndex(idx) if idx < 8 => {
                    // For compatibility purposes, switch to a brighter version
                    // of one of the standard ANSI colors when Bold is enabled.
                    // This lifts black to dark grey.
                    let idx = if attrs.intensity() == term::Intensity::Bold {
                        idx + 8
                    } else {
                        idx
                    };
                    palette.resolve_fg(term::color::ColorAttribute::PaletteIndex(idx))
                }
                _ => palette.resolve_fg(attrs.foreground),
            };

            let (fg_color, bg_color) = {
                let mut fg = fg_color;
                let mut bg = bg_color;

                if attrs.reverse() {
                    std::mem::swap(&mut fg, &mut bg);
                }

                (fg, bg)
            };

            let glyph_color = fg_color;
            let bg_color = bg_color;

            // Shape the printable text from this cluster
            let glyph_info = {
                let font = self.fonts.cached_font(style)?;
                let mut font = font.borrow_mut();
                font.shape(&cluster.text)?
            };

            for info in &glyph_info {
                let cell_idx = cluster.byte_to_cell_idx[info.cluster as usize];
                let glyph = gl_state
                    .glyph_cache
                    .borrow_mut()
                    .cached_glyph(info, style)?;

                let left = (glyph.x_offset + glyph.bearing_x) as f32;
                let top = ((self.render_metrics.cell_size.height as f64
                    + self.render_metrics.descender)
                    - (glyph.y_offset + glyph.bearing_y)) as f32;

                // underline and strikethrough
                let underline_tex_rect = gl_state
                    .util_sprites
                    .select_sprite(
                        is_highlited_hyperlink,
                        attrs.strikethrough(),
                        attrs.underline(),
                    )
                    .texture_coords();

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
                    last_cell_idx = cell_idx;

                    let (glyph_color, bg_color) = self.compute_cell_fg_bg(
                        line_idx,
                        cell_idx,
                        cursor,
                        &selection,
                        rgbcolor_to_window_color(glyph_color),
                        rgbcolor_to_window_color(bg_color),
                        palette,
                    );

                    let texture = glyph
                        .texture
                        .as_ref()
                        .unwrap_or(&gl_state.util_sprites.white_space);

                    let slice = SpriteSlice {
                        cell_idx: glyph_idx,
                        num_cells: info.num_cells as usize,
                        cell_width: self.render_metrics.cell_size.width as usize,
                        scale: glyph.scale as f32,
                        left_offset: left,
                    };

                    let pixel_rect = slice.pixel_rect(texture);
                    let texture_rect = texture.texture.to_texture_coords(pixel_rect);

                    let left = if glyph_idx == 0 { left } else { 0.0 };
                    let bottom = top + pixel_rect.max_y() as f32
                        - self.render_metrics.cell_size.height as f32;
                    let right = pixel_rect.size.width as f32 + left
                        - self.render_metrics.cell_size.width as f32;

                    let mut quad = Quad::for_cell(cell_idx, &mut vertices);

                    quad.set_fg_color(glyph_color);
                    quad.set_bg_color(bg_color);
                    quad.set_texture(texture_rect);
                    quad.set_texture_adjust(left, top, right, bottom);
                    quad.set_underline(underline_tex_rect);
                    quad.set_has_color(glyph.has_color);
                }
            }
        }

        // Clear any remaining cells to the right of the clusters we
        // found above, otherwise we leave artifacts behind.  The easiest
        // reproduction for the artifacts is to maximize the window and
        // open a vim split horizontally.  Backgrounding vim would leave
        // the right pane with its prior contents instead of showing the
        // cleared lines from the shell in the main screen.

        let white_space = gl_state.util_sprites.white_space.texture_coords();

        for cell_idx in last_cell_idx + 1..num_cols {
            // Even though we don't have a cell for these, they still
            // hold the cursor or the selection so we need to compute
            // the colors in the usual way.
            let (glyph_color, bg_color) = self.compute_cell_fg_bg(
                line_idx,
                cell_idx,
                cursor,
                &selection,
                rgbcolor_to_window_color(palette.foreground),
                rgbcolor_to_window_color(palette.background),
                palette,
            );

            let mut quad = Quad::for_cell(cell_idx, &mut vertices);

            quad.set_bg_color(bg_color);
            quad.set_fg_color(glyph_color);
            quad.set_texture(white_space);
            quad.set_texture_adjust(0., 0., 0., 0.);
            quad.set_underline(white_space);
            quad.set_has_color(false);
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn render_screen_line(
        &self,
        ctx: &mut dyn PaintContext,
        line_idx: usize,
        line: &Line,
        selection: Range<usize>,
        cursor: &CursorPosition,
        terminal: &dyn Renderable,
        palette: &ColorPalette,
    ) -> Fallible<()> {
        let (_num_rows, num_cols) = terminal.physical_dimensions();
        let current_highlight = terminal.current_highlight();

        // Break the line into clusters of cells with the same attributes
        let cell_clusters = line.cluster();
        let mut last_cell_idx = 0;
        for cluster in cell_clusters {
            let attrs = &cluster.attrs;
            let is_highlited_hyperlink = match (&attrs.hyperlink, &current_highlight) {
                (&Some(ref this), &Some(ref highlight)) => this == highlight,
                _ => false,
            };
            let style = self.fonts.match_style(attrs);

            let bg_color = palette.resolve_bg(attrs.background);
            let fg_color = match attrs.foreground {
                term::color::ColorAttribute::Default => {
                    if let Some(fg) = style.foreground {
                        fg
                    } else {
                        palette.resolve_fg(attrs.foreground)
                    }
                }
                term::color::ColorAttribute::PaletteIndex(idx) if idx < 8 => {
                    // For compatibility purposes, switch to a brighter version
                    // of one of the standard ANSI colors when Bold is enabled.
                    // This lifts black to dark grey.
                    let idx = if attrs.intensity() == term::Intensity::Bold {
                        idx + 8
                    } else {
                        idx
                    };
                    palette.resolve_fg(term::color::ColorAttribute::PaletteIndex(idx))
                }
                _ => palette.resolve_fg(attrs.foreground),
            };

            let (fg_color, bg_color) = {
                let mut fg = fg_color;
                let mut bg = bg_color;

                if attrs.reverse() {
                    std::mem::swap(&mut fg, &mut bg);
                }

                (fg, bg)
            };

            let glyph_color = rgbcolor_to_window_color(fg_color);
            let bg_color = rgbcolor_to_window_color(bg_color);

            // Shape the printable text from this cluster
            let glyph_info = {
                let font = self.fonts.cached_font(style)?;
                let mut font = font.borrow_mut();
                font.shape(&cluster.text)?
            };

            for info in &glyph_info {
                let cell_idx = cluster.byte_to_cell_idx[info.cluster as usize];
                let glyph = self.render_state.cached_software_glyph(info, style)?;

                let left = (glyph.x_offset + glyph.bearing_x) as f32;
                let top = ((self.render_metrics.cell_size.height as f64
                    + self.render_metrics.descender)
                    - (glyph.y_offset + glyph.bearing_y)) as f32;

                // underline and strikethrough
                // Figure out what we're going to draw for the underline.
                // If the current cell is part of the current URL highlight
                // then we want to show the underline.
                let underline = match (is_highlited_hyperlink, attrs.underline()) {
                    (true, Underline::None) => Underline::Single,
                    (_, underline) => underline,
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
                    last_cell_idx = cell_idx;

                    let (glyph_color, bg_color) = self.compute_cell_fg_bg(
                        line_idx,
                        cell_idx,
                        cursor,
                        &selection,
                        glyph_color,
                        bg_color,
                        palette,
                    );

                    let cell_rect = Rect::new(
                        Point::new(
                            cell_idx as isize * self.render_metrics.cell_size.width,
                            self.render_metrics.cell_size.height * line_idx as isize,
                        ),
                        self.render_metrics.cell_size,
                    );
                    ctx.clear_rect(cell_rect, bg_color);

                    match underline {
                        Underline::Single => {
                            let software = self.render_state.software();
                            let sprite = &software.util_sprites.single_underline;
                            ctx.draw_image(
                                cell_rect.origin,
                                Some(sprite.coords),
                                &*sprite.texture.image.borrow(),
                                Operator::MultiplyThenOver(glyph_color),
                            );
                        }
                        Underline::Double => {
                            let software = self.render_state.software();
                            let sprite = &software.util_sprites.double_underline;
                            ctx.draw_image(
                                cell_rect.origin,
                                Some(sprite.coords),
                                &*sprite.texture.image.borrow(),
                                Operator::MultiplyThenOver(glyph_color),
                            );
                        }
                        Underline::None => {}
                    }
                    if attrs.strikethrough() {
                        let software = self.render_state.software();
                        let sprite = &software.util_sprites.strike_through;
                        ctx.draw_image(
                            cell_rect.origin,
                            Some(sprite.coords),
                            &*sprite.texture.image.borrow(),
                            Operator::MultiplyThenOver(glyph_color),
                        );
                    }

                    if let Some(ref texture) = glyph.texture {
                        let slice = SpriteSlice {
                            cell_idx: glyph_idx,
                            num_cells: info.num_cells as usize,
                            cell_width: self.render_metrics.cell_size.width as usize,
                            scale: glyph.scale as f32,
                            left_offset: left,
                        };
                        let left = if glyph_idx == 0 { left } else { 0.0 };

                        ctx.draw_image(
                            Point::new(
                                (cell_rect.origin.x as f32 + left) as isize,
                                (cell_rect.origin.y as f32 + top) as isize,
                            ),
                            Some(slice.pixel_rect(texture)),
                            &*texture.texture.image.borrow(),
                            if glyph.has_color {
                                // For full color glyphs, always use their color.
                                // This avoids rendering a black mask when the text
                                // selection moves over the glyph
                                Operator::Over
                            } else {
                                Operator::MultiplyThenOver(glyph_color)
                            },
                        );
                    }
                }
            }
        }

        // Clear any remaining cells to the right of the clusters we
        // found above, otherwise we leave artifacts behind.  The easiest
        // reproduction for the artifacts is to maximize the window and
        // open a vim split horizontally.  Backgrounding vim would leave
        // the right pane with its prior contents instead of showing the
        // cleared lines from the shell in the main screen.

        for cell_idx in last_cell_idx + 1..num_cols {
            // Even though we don't have a cell for these, they still
            // hold the cursor or the selection so we need to compute
            // the colors in the usual way.
            let (_glyph_color, bg_color) = self.compute_cell_fg_bg(
                line_idx,
                cell_idx,
                cursor,
                &selection,
                rgbcolor_to_window_color(palette.foreground),
                rgbcolor_to_window_color(palette.background),
                palette,
            );

            let cell_rect = Rect::new(
                Point::new(
                    cell_idx as isize * self.render_metrics.cell_size.width,
                    self.render_metrics.cell_size.height * line_idx as isize,
                ),
                self.render_metrics.cell_size,
            );
            ctx.clear_rect(cell_rect, bg_color);
        }

        // Fill any marginal area to the right of the last cell
        let pixel_width_of_cells = num_cols * self.render_metrics.cell_size.width as usize;
        ctx.clear_rect(
            Rect::new(
                Point::new(
                    pixel_width_of_cells as isize,
                    self.render_metrics.cell_size.height * line_idx as isize,
                ),
                Size::new(
                    (self.dimensions.pixel_width - pixel_width_of_cells) as isize,
                    self.render_metrics.cell_size.height,
                ),
            ),
            rgbcolor_to_window_color(palette.background),
        );

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn compute_cell_fg_bg(
        &self,
        line_idx: usize,
        cell_idx: usize,
        cursor: &CursorPosition,
        selection: &Range<usize>,
        fg_color: Color,
        bg_color: Color,
        palette: &ColorPalette,
    ) -> (Color, Color) {
        let selected = selection.contains(&cell_idx);
        let is_cursor = line_idx as i64 == cursor.y && cursor.x == cell_idx;

        let (fg_color, bg_color) = match (selected, is_cursor) {
            // Normally, render the cell as configured
            (false, false) => (fg_color, bg_color),
            // Cursor cell overrides colors
            (_, true) => (
                rgbcolor_to_window_color(palette.cursor_fg),
                rgbcolor_to_window_color(palette.cursor_bg),
            ),
            // Selected text overrides colors
            (true, false) => (
                rgbcolor_to_window_color(palette.selection_fg),
                rgbcolor_to_window_color(palette.selection_bg),
            ),
        };

        (fg_color, bg_color)
    }
}

fn rgbcolor_to_window_color(color: RgbColor) -> Color {
    Color::rgba(color.red, color.green, color.blue, 0xff)
}

fn window_mods_to_termwiz_mods(modifiers: ::window::Modifiers) -> termwiz::input::Modifiers {
    let mut result = termwiz::input::Modifiers::NONE;
    if modifiers.contains(::window::Modifiers::SHIFT) {
        result.insert(termwiz::input::Modifiers::SHIFT);
    }
    if modifiers.contains(::window::Modifiers::ALT) {
        result.insert(termwiz::input::Modifiers::ALT);
    }
    if modifiers.contains(::window::Modifiers::CTRL) {
        result.insert(termwiz::input::Modifiers::CTRL);
    }
    if modifiers.contains(::window::Modifiers::SUPER) {
        result.insert(termwiz::input::Modifiers::SUPER);
    }
    result
}
