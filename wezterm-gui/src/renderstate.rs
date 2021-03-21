use super::glyphcache::GlyphCache;
use super::quad::*;
use super::utilsprites::{RenderMetrics, UtilSprites};
use ::window::bitmaps::atlas::OutOfTextureSpace;
use ::window::glium::backend::Context as GliumContext;
use ::window::glium::texture::SrgbTexture2d;
use ::window::glium::{IndexBuffer, VertexBuffer};
use ::window::*;
use config::ConfigHandle;
use std::cell::RefCell;
use std::rc::Rc;
use wezterm_font::FontConfiguration;

pub struct TripleVertexBuffer {
    pub index: usize,
    pub bufs: [VertexBuffer<Vertex>; 3],
}

pub struct RenderState {
    pub context: Rc<GliumContext>,
    pub glyph_cache: RefCell<GlyphCache<SrgbTexture2d>>,
    pub util_sprites: UtilSprites<SrgbTexture2d>,
    pub background_prog: glium::Program,
    pub line_prog: glium::Program,
    pub glyph_prog: glium::Program,
    pub glyph_vertex_buffer: RefCell<TripleVertexBuffer>,
    pub glyph_index_buffer: IndexBuffer<u32>,
    pub quads: Quads,
}

impl RenderState {
    pub fn new(
        config: &ConfigHandle,
        context: Rc<GliumContext>,
        fonts: &Rc<FontConfiguration>,
        metrics: &RenderMetrics,
        mut atlas_size: usize,
        pixel_width: usize,
        pixel_height: usize,
    ) -> anyhow::Result<Self> {
        let early_stage_srgb = false;
        let last_stage_srgb = false;

        loop {
            let glyph_cache =
                RefCell::new(GlyphCache::new_gl(&context, fonts, atlas_size, metrics)?);
            let result = UtilSprites::new(&mut *glyph_cache.borrow_mut(), metrics);
            match result {
                Ok(util_sprites) => {
                    let background_prog =
                        Self::compile_prog(&context, early_stage_srgb, Self::background_shader)?;
                    let line_prog =
                        Self::compile_prog(&context, early_stage_srgb, Self::line_shader)?;
                    let glyph_prog =
                        Self::compile_prog(&context, last_stage_srgb, Self::glyph_shader)?;

                    let (glyph_vertex_buffer, glyph_index_buffer, quads) = Self::compute_vertices(
                        config,
                        &context,
                        metrics,
                        pixel_width as f32,
                        pixel_height as f32,
                    )?;

                    return Ok(Self {
                        context,
                        glyph_cache,
                        util_sprites,
                        background_prog,
                        line_prog,
                        glyph_prog,
                        glyph_vertex_buffer: RefCell::new(glyph_vertex_buffer),
                        glyph_index_buffer,
                        quads,
                    });
                }
                Err(OutOfTextureSpace {
                    size: Some(size), ..
                }) => {
                    atlas_size = size;
                }
                Err(OutOfTextureSpace { size: None, .. }) => {
                    anyhow::bail!("requested texture size is impossible!?")
                }
            };
        }
    }

    fn compile_prog(
        context: &Rc<GliumContext>,
        outputs_srgb: bool,
        fragment_shader: fn(&str) -> (String, String),
    ) -> anyhow::Result<glium::Program> {
        let mut errors = vec![];
        for version in &["330", "300 es"] {
            let (vertex_shader, fragment_shader) = fragment_shader(version);
            let source = glium::program::ProgramCreationInput::SourceCode {
                vertex_shader: &vertex_shader,
                fragment_shader: &fragment_shader,
                outputs_srgb,
                tessellation_control_shader: None,
                tessellation_evaluation_shader: None,
                transform_feedback_varyings: None,
                uses_point_size: false,
                geometry_shader: None,
            };
            log::trace!("compiling a prog with version {}", version);
            match glium::Program::new(context, source) {
                Ok(prog) => {
                    return Ok(prog);
                }
                Err(err) => errors.push(err.to_string()),
            };
        }

        anyhow::bail!("Failed to compile shaders: {}", errors.join("\n"))
    }

    pub fn advise_of_window_size_change(
        &mut self,
        config: &ConfigHandle,
        metrics: &RenderMetrics,
        pixel_width: usize,
        pixel_height: usize,
    ) -> anyhow::Result<()> {
        let (glyph_vertex_buffer, glyph_index_buffer, quads) = Self::compute_vertices(
            config,
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

    fn glyph_shader(version: &str) -> (String, String) {
        (
            format!(
                "#version {}\n{}\n{}",
                version,
                include_str!("vertex-common.glsl"),
                include_str!("glyph-vertex.glsl")
            ),
            format!(
                "#version {}\n{}\n{}",
                version,
                include_str!("fragment-common.glsl"),
                include_str!("glyph-frag.glsl")
            ),
        )
    }

    fn line_shader(version: &str) -> (String, String) {
        (
            format!(
                "#version {}\n{}\n{}",
                version,
                include_str!("vertex-common.glsl"),
                include_str!("line-vertex.glsl")
            ),
            format!(
                "#version {}\n{}\n{}",
                version,
                include_str!("fragment-common.glsl"),
                include_str!("line-frag.glsl")
            ),
        )
    }

    fn background_shader(version: &str) -> (String, String) {
        (
            format!(
                "#version {}\n{}\n{}",
                version,
                include_str!("vertex-common.glsl"),
                include_str!("background-vertex.glsl")
            ),
            format!(
                "#version {}\n{}\n{}",
                version,
                include_str!("fragment-common.glsl"),
                include_str!("background-frag.glsl")
            ),
        )
    }

    /// Compute a vertex buffer to hold the quads that comprise the visible
    /// portion of the screen.   We recreate this when the screen is resized.
    /// The idea is that we want to minimize any heavy lifting and computation
    /// and instead just poke some attributes into the offset that corresponds
    /// to a changed cell when we need to repaint the screen, and then just
    /// let the GPU figure out the rest.
    fn compute_vertices(
        config: &ConfigHandle,
        context: &Rc<GliumContext>,
        metrics: &RenderMetrics,
        width: f32,
        height: f32,
    ) -> anyhow::Result<(TripleVertexBuffer, IndexBuffer<u32>, Quads)> {
        let cell_width = metrics.cell_size.width as f32;
        let cell_height = metrics.cell_size.height as f32;
        let mut verts = Vec::new();
        let mut indices = Vec::new();

        let padding_right = super::termwindow::resize::effective_right_padding(&config, metrics);
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

        for y in 0..=num_rows {
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

        // And a quad for the scrollbar thumb
        quads.scroll_thumb = define_quad(0.0, 0.0, 0.0, 0.0) as usize;

        let buffer = TripleVertexBuffer {
            index: 0,
            bufs: [
                VertexBuffer::dynamic(context, &verts)?,
                VertexBuffer::dynamic(context, &verts)?,
                VertexBuffer::dynamic(context, &verts)?,
            ],
        };

        Ok((
            buffer,
            IndexBuffer::new(
                context,
                glium::index::PrimitiveType::TrianglesList,
                &indices,
            )?,
            quads,
        ))
    }

    pub fn clear_texture_atlas(&mut self, metrics: &RenderMetrics) -> anyhow::Result<()> {
        let mut glyph_cache = self.glyph_cache.borrow_mut();
        glyph_cache.clear();
        self.util_sprites = UtilSprites::new(&mut glyph_cache, metrics)?;
        Ok(())
    }

    pub fn recreate_texture_atlas(
        &mut self,
        fonts: &Rc<FontConfiguration>,
        metrics: &RenderMetrics,
        size: Option<usize>,
    ) -> anyhow::Result<()> {
        // We make a a couple of passes at resizing; if the user has selected a large
        // font size (or a large scaling factor) then the `size==None` case will not
        // be able to fit the initial utility glyphs and apply_scale_change won't
        // be able to deal with that error situation.  Rather than make every
        // caller know how to deal with OutOfTextureSpace we try to absorb
        // and accomodate that here.
        let mut size = size;
        let mut attempt = 10;
        loop {
            match self.recreate_texture_atlas_impl(fonts, metrics, size) {
                Ok(_) => return Ok(()),
                Err(err) => {
                    attempt -= 1;
                    if attempt == 0 {
                        return Err(err);
                    }

                    if let Some(&OutOfTextureSpace {
                        size: Some(needed_size),
                        ..
                    }) = err.downcast_ref::<OutOfTextureSpace>()
                    {
                        size.replace(needed_size);
                        continue;
                    }

                    return Err(err);
                }
            }
        }
    }

    fn recreate_texture_atlas_impl(
        &mut self,
        fonts: &Rc<FontConfiguration>,
        metrics: &RenderMetrics,
        size: Option<usize>,
    ) -> anyhow::Result<()> {
        let size = size.unwrap_or_else(|| self.glyph_cache.borrow().atlas.size());
        let mut new_glyph_cache = GlyphCache::new_gl(&self.context, fonts, size, metrics)?;
        self.util_sprites = UtilSprites::new(&mut new_glyph_cache, metrics)?;

        let mut glyph_cache = self.glyph_cache.borrow_mut();

        // Steal the decoded image cache; without this, any animating gifs
        // would reset back to frame 0 each time we filled the texture
        std::mem::swap(
            &mut glyph_cache.image_cache,
            &mut new_glyph_cache.image_cache,
        );

        *glyph_cache = new_glyph_cache;
        Ok(())
    }
}
