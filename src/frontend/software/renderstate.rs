use super::glyphcache::{CachedGlyph, GlyphCache};
use super::utilsprites::{RenderMetrics, UtilSprites};
use crate::config::TextStyle;
use crate::font::{FontConfiguration, GlyphInfo};
use ::window::bitmaps::ImageTexture;
use ::window::glium::backend::Context as GliumContext;
use ::window::glium::texture::SrgbTexture2d;
use ::window::glium::{IndexBuffer, VertexBuffer};
use ::window::*;
use failure::Fallible;
use std::cell::RefCell;
use std::rc::Rc;

use super::quad::Vertex;

pub struct SoftwareRenderState {
    pub glyph_cache: RefCell<GlyphCache<ImageTexture>>,
    pub util_sprites: UtilSprites<ImageTexture>,
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

pub struct OpenGLRenderState {
    pub context: Rc<GliumContext>,
    pub glyph_cache: RefCell<GlyphCache<SrgbTexture2d>>,
    pub util_sprites: UtilSprites<SrgbTexture2d>,
    pub program: glium::Program,
    pub glyph_vertex_buffer: RefCell<VertexBuffer<Vertex>>,
    pub glyph_index_buffer: IndexBuffer<u32>,
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
