use super::glyphcache::{CachedGlyph, GlyphCache};
use super::quad::*;
use super::utilsprites::{RenderMetrics, UtilSprites};
use crate::font::{FontConfiguration, GlyphInfo};
use ::window::bitmaps::ImageTexture;
use ::window::glium::backend::Context as GliumContext;
use ::window::glium::texture::SrgbTexture2d;
use ::window::glium::{IndexBuffer, VertexBuffer};
use ::window::*;
use anyhow::{anyhow, bail};
use config::{configuration, TextStyle};
use std::cell::RefCell;
use std::rc::Rc;

pub struct SoftwareRenderState {
    pub glyph_cache: RefCell<GlyphCache<ImageTexture>>,
    pub util_sprites: UtilSprites<ImageTexture>,
}

impl SoftwareRenderState {
    pub fn new(
        fonts: &Rc<FontConfiguration>,
        metrics: &RenderMetrics,
        size: usize,
    ) -> anyhow::Result<Self> {
        let glyph_cache = RefCell::new(GlyphCache::new(fonts, size));
        let util_sprites = UtilSprites::new(&mut glyph_cache.borrow_mut(), metrics)?;
        Ok(Self {
            glyph_cache,
            util_sprites,
        })
    }
}

pub struct OpenGLRenderState {
    pub context: Rc<GliumContext>,
    pub glyph_cache: RefCell<GlyphCache<SrgbTexture2d>>,
    pub util_sprites: UtilSprites<SrgbTexture2d>,
    pub program: glium::Program,
    pub glyph_vertex_buffer: RefCell<VertexBuffer<Vertex>>,
    pub glyph_index_buffer: IndexBuffer<u32>,
    pub quads: Quads,
}

impl OpenGLRenderState {
    pub fn new(
        context: Rc<GliumContext>,
        fonts: &Rc<FontConfiguration>,
        metrics: &RenderMetrics,
        size: usize,
        pixel_width: usize,
        pixel_height: usize,
    ) -> anyhow::Result<Self> {
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
            log::info!("compiling a prog with version {}", version);
            match glium::Program::new(&context, source) {
                Ok(prog) => {
                    program = Some(prog);
                    break;
                }
                Err(err) => errors.push(err.to_string()),
            };
        }

        let program =
            program.ok_or_else(|| anyhow!("Failed to compile shaders: {}", errors.join("\n")))?;

        let (glyph_vertex_buffer, glyph_index_buffer, quads) =
            Self::compute_vertices(&context, metrics, pixel_width as f32, pixel_height as f32)?;

        Ok(Self {
            context,
            glyph_cache,
            util_sprites,
            program,
            glyph_vertex_buffer: RefCell::new(glyph_vertex_buffer),
            glyph_index_buffer,
            quads,
        })
    }

    pub fn advise_of_window_size_change(
        &mut self,
        metrics: &RenderMetrics,
        pixel_width: usize,
        pixel_height: usize,
    ) -> anyhow::Result<()> {
        let (glyph_vertex_buffer, glyph_index_buffer, quads) = Self::compute_vertices(
            &self.context,
            metrics,
            pixel_width as f32,
            pixel_height as f32,
        )?;

        *self.glyph_vertex_buffer.borrow_mut() = glyph_vertex_buffer;
        self.glyph_index_buffer = glyph_index_buffer;
        self.quads = quads;
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
    /// The idea is that we want to minimize any heavy lifting and computation
    /// and instead just poke some attributes into the offset that corresponds
    /// to a changed cell when we need to repaint the screen, and then just
    /// let the GPU figure out the rest.
    fn compute_vertices(
        context: &Rc<GliumContext>,
        metrics: &RenderMetrics,
        width: f32,
        height: f32,
    ) -> anyhow::Result<(VertexBuffer<Vertex>, IndexBuffer<u32>, Quads)> {
        let cell_width = metrics.cell_size.width as f32;
        let cell_height = metrics.cell_size.height as f32;
        let mut verts = Vec::new();
        let mut indices = Vec::new();

        let config = configuration();
        let padding_right = super::termwindow::effective_right_padding(&config, metrics);
        let avail_width =
            (width as usize).saturating_sub((config.window_padding.left + padding_right) as usize);
        let avail_height = (height as usize)
            .saturating_sub((config.window_padding.top + config.window_padding.bottom) as usize);

        let num_cols = avail_width as usize / cell_width as usize;
        let num_rows = avail_height as usize / cell_height as usize;

        let padding_left = config.window_padding.left as f32;
        let padding_top = config.window_padding.top as f32;

        log::debug!(
            "compute_vertices {}x{} {}x{} padding={} {}",
            num_cols,
            num_rows,
            width,
            height,
            padding_left,
            padding_top
        );

        let mut quads = Quads::default();
        quads.cols = num_cols;

        let mut define_quad = |left, top, right, bottom| -> u32 {
            // Remember starting index for this position
            let idx = verts.len() as u32;

            verts.push(Vertex {
                // Top left
                position: (left, top),
                ..Default::default()
            });
            verts.push(Vertex {
                // Top Right
                position: (right, top),
                ..Default::default()
            });
            verts.push(Vertex {
                // Bottom Left
                position: (left, bottom),
                ..Default::default()
            });
            verts.push(Vertex {
                // Bottom Right
                position: (right, bottom),
                ..Default::default()
            });

            // Emit two triangles to form the glyph quad
            indices.push(idx + V_TOP_LEFT as u32);
            indices.push(idx + V_TOP_RIGHT as u32);
            indices.push(idx + V_BOT_LEFT as u32);

            indices.push(idx + V_TOP_RIGHT as u32);
            indices.push(idx + V_BOT_LEFT as u32);
            indices.push(idx + V_BOT_RIGHT as u32);

            idx
        };

        // Background image fills the entire window background
        quads.background_image =
            define_quad(width / -2.0, height / -2.0, width / 2.0, height / 2.0) as usize;

        for y in 0..num_rows {
            let y_pos = (height / -2.0) + (y as f32 * cell_height) + padding_top;

            for x in 0..num_cols {
                let x_pos = (width / -2.0) + (x as f32 * cell_width) + padding_left;

                let idx = define_quad(x_pos, y_pos, x_pos + cell_width, y_pos + cell_height);
                if x == 0 {
                    // build row -> vertex mapping
                    quads.row_starts.push(idx as usize);
                }
            }
        }

        {
            // And a quad for the scrollbar thumb
            let x_pos = (width / 2.0) - cell_width;
            let y_pos = (height / -2.0) + padding_top;
            let thumb_width = cell_width;
            let thumb_height = height;

            quads.scroll_thumb =
                define_quad(x_pos, y_pos, x_pos + thumb_width, y_pos + thumb_height) as usize;
        }

        Ok((
            VertexBuffer::dynamic(context, &verts)?,
            IndexBuffer::new(
                context,
                glium::index::PrimitiveType::TrianglesList,
                &indices,
            )?,
            quads,
        ))
    }
}

#[allow(clippy::large_enum_variant)]
pub enum RenderState {
    Software(SoftwareRenderState),
    GL(OpenGLRenderState),
}

impl RenderState {
    pub fn recreate_texture_atlas(
        &mut self,
        fonts: &Rc<FontConfiguration>,
        metrics: &RenderMetrics,
        size: Option<usize>,
    ) -> anyhow::Result<()> {
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
    ) -> anyhow::Result<()> {
        if let RenderState::GL(gl) = self {
            gl.advise_of_window_size_change(metrics, pixel_width, pixel_height)?;
        }
        Ok(())
    }

    pub fn cached_software_glyph(
        &self,
        info: &GlyphInfo,
        style: &TextStyle,
    ) -> anyhow::Result<Rc<CachedGlyph<ImageTexture>>> {
        if let RenderState::Software(software) = self {
            software.glyph_cache.borrow_mut().cached_glyph(info, style)
        } else {
            bail!("attempted to call cached_software_glyph when in gl mode")
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
