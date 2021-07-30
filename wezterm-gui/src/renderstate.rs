use super::glyphcache::GlyphCache;
use super::quad::*;
use super::utilsprites::{RenderMetrics, UtilSprites};
use ::window::bitmaps::atlas::OutOfTextureSpace;
use ::window::glium::backend::Context as GliumContext;
use ::window::glium::buffer::Mapping;
use ::window::glium::texture::SrgbTexture2d;
use ::window::glium::{IndexBuffer, VertexBuffer};
use ::window::*;
use std::cell::{Ref, RefCell, RefMut};
use std::rc::Rc;
use wezterm_font::FontConfiguration;

const INDICES_PER_CELL: usize = 6;

pub struct MappedQuads<'a> {
    mapping: Mapping<'a, [Vertex]>,
    next: RefMut<'a, usize>,
    capacity: usize,
}

impl<'a> MappedQuads<'a> {
    pub fn allocate<'b>(&'b mut self) -> anyhow::Result<Quad<'b>> {
        let idx = *self.next;
        if idx >= self.capacity {
            anyhow::bail!("not enough quads! do something like OutOfTextureSpace");
        }
        *self.next += 1;

        let idx = idx * VERTICES_PER_CELL;
        let mut quad = Quad {
            vert: &mut self.mapping[idx..idx + VERTICES_PER_CELL],
        };

        quad.set_texture_adjust(0., 0., 0., 0.);
        quad.set_has_color(false);

        Ok(quad)
    }
}

pub struct TripleVertexBuffer {
    pub index: RefCell<usize>,
    pub bufs: RefCell<[VertexBuffer<Vertex>; 3]>,
    pub indices: IndexBuffer<u32>,
    pub capacity: usize,
    pub next_quad: RefCell<usize>,
}

impl TripleVertexBuffer {
    pub fn clear_quad_allocation(&self) {
        *self.next_quad.borrow_mut() = 0;
    }

    pub fn vertex_index_count(&self) -> (usize, usize) {
        let num_quads = *self.next_quad.borrow();
        (num_quads * VERTICES_PER_CELL, num_quads * INDICES_PER_CELL)
    }

    pub fn map<'a>(&'a self, bufs: &'a mut RefMut<VertexBuffer<Vertex>>) -> MappedQuads<'a> {
        let mapping = bufs.slice_mut(..).expect("to map vertex buffer").map();
        MappedQuads {
            mapping,
            next: self.next_quad.borrow_mut(),
            capacity: self.capacity,
        }
    }

    pub fn current_vb(&self) -> Ref<VertexBuffer<Vertex>> {
        let index = *self.index.borrow();
        let bufs = self.bufs.borrow();
        Ref::map(bufs, |bufs| &bufs[index])
    }

    pub fn current_vb_mut(&self) -> RefMut<VertexBuffer<Vertex>> {
        let index = *self.index.borrow();
        let bufs = self.bufs.borrow_mut();
        RefMut::map(bufs, |bufs| &mut bufs[index])
    }

    pub fn next_index(&self) {
        let mut index = self.index.borrow_mut();
        *index += 1;
        if *index >= 3 {
            *index = 0;
        }
    }
}

pub struct RenderState {
    pub context: Rc<GliumContext>,
    pub glyph_cache: RefCell<GlyphCache<SrgbTexture2d>>,
    pub util_sprites: UtilSprites<SrgbTexture2d>,
    pub background_prog: glium::Program,
    pub line_prog: glium::Program,
    pub glyph_prog: glium::Program,
    pub img_prog: glium::Program,
    pub glyph_vertex_buffer: TripleVertexBuffer,
}

impl RenderState {
    pub fn new(
        context: Rc<GliumContext>,
        fonts: &Rc<FontConfiguration>,
        metrics: &RenderMetrics,
        mut atlas_size: usize,
        pixel_width: usize,
        pixel_height: usize,
    ) -> anyhow::Result<Self> {
        loop {
            let glyph_cache =
                RefCell::new(GlyphCache::new_gl(&context, fonts, atlas_size, metrics)?);
            let result = UtilSprites::new(&mut *glyph_cache.borrow_mut(), metrics);
            match result {
                Ok(util_sprites) => {
                    let do_gamma = cfg!(target_os = "macos");

                    let background_prog =
                        Self::compile_prog(&context, do_gamma, Self::background_shader)?;
                    let line_prog = Self::compile_prog(&context, do_gamma, Self::line_shader)?;

                    let glyph_prog = Self::compile_prog(&context, do_gamma, Self::glyph_shader)?;
                    // Last prog outputs srgb for gamma correction
                    let img_prog = Self::compile_prog(&context, true, Self::img_shader)?;

                    let glyph_vertex_buffer = Self::compute_vertices(
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
                        img_prog,
                        glyph_vertex_buffer,
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
        metrics: &RenderMetrics,
        pixel_width: usize,
        pixel_height: usize,
    ) -> anyhow::Result<()> {
        let glyph_vertex_buffer = Self::compute_vertices(
            &self.context,
            metrics,
            pixel_width as f32,
            pixel_height as f32,
        )?;

        self.glyph_vertex_buffer = glyph_vertex_buffer;
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

    fn img_shader(version: &str) -> (String, String) {
        (
            format!(
                "#version {}\n{}\n{}",
                version,
                include_str!("vertex-common.glsl"),
                include_str!("img-vertex.glsl")
            ),
            format!(
                "#version {}\n{}\n{}",
                version,
                include_str!("fragment-common.glsl"),
                include_str!("img-frag.glsl")
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
        context: &Rc<GliumContext>,
        metrics: &RenderMetrics,
        width: f32,
        height: f32,
    ) -> anyhow::Result<TripleVertexBuffer> {
        let cell_width = metrics.cell_size.width as f32;
        let cell_height = metrics.cell_size.height as f32;

        let num_cols = width as usize / cell_width as usize;
        let num_rows = height as usize / cell_height as usize;

        log::debug!(
            "compute_vertices {}x{} {}x{}",
            num_cols,
            num_rows,
            width,
            height,
        );

        let num_quads = num_cols * num_rows + 2 /* bg image, scroll thumb */;
        let verts = vec![Vertex::default(); num_quads * VERTICES_PER_CELL];
        let mut indices = vec![];
        indices.reserve(num_quads * INDICES_PER_CELL);

        for q in 0..num_quads {
            let idx = (q * VERTICES_PER_CELL) as u32;

            // Emit two triangles to form the glyph quad
            indices.push(idx + V_TOP_LEFT as u32);
            indices.push(idx + V_TOP_RIGHT as u32);
            indices.push(idx + V_BOT_LEFT as u32);

            indices.push(idx + V_TOP_RIGHT as u32);
            indices.push(idx + V_BOT_LEFT as u32);
            indices.push(idx + V_BOT_RIGHT as u32);
        }

        let buffer = TripleVertexBuffer {
            index: RefCell::new(0),
            bufs: RefCell::new([
                VertexBuffer::dynamic(context, &verts)?,
                VertexBuffer::dynamic(context, &verts)?,
                VertexBuffer::dynamic(context, &verts)?,
            ]),
            capacity: verts.len() / VERTICES_PER_CELL,
            indices: IndexBuffer::new(
                context,
                glium::index::PrimitiveType::TrianglesList,
                &indices,
            )?,
            next_quad: RefCell::new(0),
        };

        Ok(buffer)
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
