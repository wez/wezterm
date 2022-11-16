// Clippy hates the implement_vertex macro and won't let me scope
// this warning to its use
#![allow(clippy::unneeded_field_pattern)]

use crate::renderstate::BorrowedLayers;
use ::window::bitmaps::TextureRect;
use ::window::color::LinearRgba;
use config::HsbTransform;

/// Each cell is composed of two triangles built from 4 vertices.
/// The buffer is organized row by row.
pub const VERTICES_PER_CELL: usize = 4;
pub const V_TOP_LEFT: usize = 0;
pub const V_TOP_RIGHT: usize = 1;
pub const V_BOT_LEFT: usize = 2;
pub const V_BOT_RIGHT: usize = 3;

#[repr(C)]
#[derive(Copy, Clone, Default, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    // Physical position of the corner of the character cell
    pub position: [f32; 2],
    // glyph texture
    pub tex: [f32; 2],
    pub fg_color: [f32; 4],
    pub alt_color: [f32; 4],
    pub hsv: [f32; 3],
    // We use a float for this because I can't get
    // bool or integer values to work:
    // "bool can't be an in in the vertex shader"
    //
    // has_color is effectively an enum with these
    // possible values:
    // 0.0 -> a regular monochrome text glyph
    // 1.0 -> a color emoji glyph
    // 2.0 -> a full color texture attached as the
    //        background image of the window
    // 3.0 -> like 2.0, except that instead of an
    //        image, we use the solid bg color
    pub has_color: f32,
    pub mix_value: f32,
}
::window::glium::implement_vertex!(
    Vertex, position, tex, fg_color, alt_color, hsv, has_color, mix_value
);

pub trait QuadTrait {
    /// Assign the texture coordinates
    fn set_texture(&mut self, coords: TextureRect) {
        let x1 = coords.min_x();
        let x2 = coords.max_x();
        let y1 = coords.min_y();
        let y2 = coords.max_y();
        self.set_texture_discrete(x1, x2, y1, y2);
    }
    fn set_texture_discrete(&mut self, x1: f32, x2: f32, y1: f32, y2: f32);
    fn set_has_color_impl(&mut self, has_color: f32);

    /// Set the color glyph "flag"
    fn set_has_color(&mut self, has_color: bool) {
        self.set_has_color_impl(if has_color { 1. } else { 0. });
    }

    /// Mark as a grayscale polyquad; color and alpha will be
    /// multipled with those in the texture
    fn set_grayscale(&mut self) {
        self.set_has_color_impl(4.0);
    }

    /// Mark this quad as a background image.
    /// Mutually exclusive with set_has_color.
    fn set_is_background_image(&mut self) {
        self.set_has_color_impl(2.0);
    }

    fn set_is_background(&mut self) {
        self.set_has_color_impl(3.0);
    }

    fn set_fg_color(&mut self, color: LinearRgba);

    /// Must be called after set_fg_color
    fn set_alt_color_and_mix_value(&mut self, color: LinearRgba, mix_value: f32);

    fn set_hsv(&mut self, hsv: Option<HsbTransform>);
    fn set_position(&mut self, left: f32, top: f32, right: f32, bottom: f32);
}

pub enum QuadImpl<'a> {
    Vert(Quad<'a>),
    Boxed(&'a mut BoxedQuad),
}

impl<'a> QuadTrait for QuadImpl<'a> {
    fn set_texture_discrete(&mut self, x1: f32, x2: f32, y1: f32, y2: f32) {
        match self {
            Self::Vert(q) => q.set_texture_discrete(x1, x2, y1, y2),
            Self::Boxed(q) => q.set_texture_discrete(x1, x2, y1, y2),
        }
    }

    fn set_has_color_impl(&mut self, has_color: f32) {
        match self {
            Self::Vert(q) => q.set_has_color_impl(has_color),
            Self::Boxed(q) => q.set_has_color_impl(has_color),
        }
    }

    fn set_fg_color(&mut self, color: LinearRgba) {
        match self {
            Self::Vert(q) => q.set_fg_color(color),
            Self::Boxed(q) => q.set_fg_color(color),
        }
    }

    fn set_alt_color_and_mix_value(&mut self, color: LinearRgba, mix_value: f32) {
        match self {
            Self::Vert(q) => q.set_alt_color_and_mix_value(color, mix_value),
            Self::Boxed(q) => q.set_alt_color_and_mix_value(color, mix_value),
        }
    }

    fn set_hsv(&mut self, hsv: Option<HsbTransform>) {
        match self {
            Self::Vert(q) => q.set_hsv(hsv),
            Self::Boxed(q) => q.set_hsv(hsv),
        }
    }

    fn set_position(&mut self, left: f32, top: f32, right: f32, bottom: f32) {
        match self {
            Self::Vert(q) => q.set_position(left, top, right, bottom),
            Self::Boxed(q) => q.set_position(left, top, right, bottom),
        }
    }
}

/// A helper for updating the 4 vertices that compose a glyph cell
pub struct Quad<'a> {
    pub(crate) vert: &'a mut [Vertex],
}

impl<'a> QuadTrait for Quad<'a> {
    fn set_texture_discrete(&mut self, x1: f32, x2: f32, y1: f32, y2: f32) {
        self.vert[V_TOP_LEFT].tex = [x1, y1];
        self.vert[V_TOP_RIGHT].tex = [x2, y1];
        self.vert[V_BOT_LEFT].tex = [x1, y2];
        self.vert[V_BOT_RIGHT].tex = [x2, y2];
    }

    fn set_has_color_impl(&mut self, has_color: f32) {
        for v in self.vert.iter_mut() {
            v.has_color = has_color;
        }
    }

    fn set_fg_color(&mut self, color: LinearRgba) {
        for v in self.vert.iter_mut() {
            v.fg_color = color.into();
        }
        self.set_alt_color_and_mix_value(color, 0.);
    }

    /// Must be called after set_fg_color
    fn set_alt_color_and_mix_value(&mut self, color: LinearRgba, mix_value: f32) {
        for v in self.vert.iter_mut() {
            v.alt_color = color.into();
            v.mix_value = mix_value;
        }
    }

    fn set_hsv(&mut self, hsv: Option<HsbTransform>) {
        let (h, s, v) = hsv
            .map(|t| (t.hue, t.saturation, t.brightness))
            .unwrap_or((1., 1., 1.));
        for vert in self.vert.iter_mut() {
            vert.hsv = [h, s, v];
        }
    }

    fn set_position(&mut self, left: f32, top: f32, right: f32, bottom: f32) {
        self.vert[V_TOP_LEFT].position = [left, top];
        self.vert[V_TOP_RIGHT].position = [right, top];
        self.vert[V_BOT_LEFT].position = [left, bottom];
        self.vert[V_BOT_RIGHT].position = [right, bottom];
    }
}

pub trait QuadAllocator {
    fn allocate(&mut self) -> anyhow::Result<QuadImpl>;
    fn extend_with(&mut self, vertices: &[Vertex]);
}

pub trait TripleLayerQuadAllocatorTrait {
    fn allocate(&mut self, layer_num: usize) -> anyhow::Result<QuadImpl>;
    fn extend_with(&mut self, layer_num: usize, vertices: &[Vertex]);
}

/// We prefer to allocate a quad at a time for HeapQuadAllocator
/// because we tend to end up with fairly large arrays of Vertex
/// and the total amount of contiguous memory is in the MB range,
/// which is a bit gnarly to reallocate, and can waste several MB
/// in unused capacity
#[derive(Default)]
pub struct BoxedQuad {
    position: (f32, f32, f32, f32),
    fg_color: [f32; 4],
    alt_color: [f32; 4],
    tex: (f32, f32, f32, f32),
    hsv: [f32; 3],
    has_color: f32,
    mix_value: f32,
}

impl QuadTrait for BoxedQuad {
    fn set_texture_discrete(&mut self, x1: f32, x2: f32, y1: f32, y2: f32) {
        self.tex = (x1, x2, y1, y2);
    }

    fn set_has_color_impl(&mut self, has_color: f32) {
        self.has_color = has_color;
    }

    fn set_fg_color(&mut self, color: LinearRgba) {
        self.fg_color = color.into();
    }
    fn set_alt_color_and_mix_value(&mut self, color: LinearRgba, mix_value: f32) {
        self.alt_color = color.into();
        self.mix_value = mix_value;
    }
    fn set_hsv(&mut self, hsv: Option<HsbTransform>) {
        let (h, s, v) = hsv
            .map(|t| (t.hue, t.saturation, t.brightness))
            .unwrap_or((1., 1., 1.));
        self.hsv = [h, s, v];
    }

    fn set_position(&mut self, left: f32, top: f32, right: f32, bottom: f32) {
        self.position = (left, top, right, bottom);
    }
}

impl BoxedQuad {
    fn from_vertices(verts: &[Vertex; VERTICES_PER_CELL]) -> Self {
        let [x1, y1] = verts[V_TOP_LEFT].tex;
        let [x2, y2] = verts[V_BOT_RIGHT].tex;

        let [left, top] = verts[V_TOP_LEFT].position;
        let [right, bottom] = verts[V_BOT_RIGHT].position;
        Self {
            tex: (x1, x2, y1, y2),
            position: (left, top, right, bottom),
            has_color: verts[V_TOP_LEFT].has_color,
            alt_color: verts[V_TOP_LEFT].alt_color,
            fg_color: verts[V_TOP_LEFT].fg_color,
            hsv: verts[V_TOP_LEFT].hsv,
            mix_value: verts[V_TOP_LEFT].mix_value,
        }
    }

    fn to_vertices(&self) -> [Vertex; VERTICES_PER_CELL] {
        let mut vert: [Vertex; VERTICES_PER_CELL] = Default::default();
        let mut quad = Quad { vert: &mut vert };

        let (x1, x2, y1, y2) = self.tex;
        quad.set_texture_discrete(x1, x2, y1, y2);

        let (left, top, right, bottom) = self.position;
        quad.set_position(left, top, right, bottom);

        quad.set_has_color_impl(self.has_color);
        let [hue, saturation, brightness] = self.hsv;
        quad.set_hsv(Some(HsbTransform {
            hue,
            saturation,
            brightness,
        }));
        quad.set_fg_color(LinearRgba::with_components(
            self.fg_color[0],
            self.fg_color[1],
            self.fg_color[2],
            self.fg_color[3],
        ));
        quad.set_alt_color_and_mix_value(self.alt_color.into(), self.mix_value);

        vert
    }
}

#[derive(Default)]
pub struct HeapQuadAllocator {
    layer0: Vec<Box<BoxedQuad>>,
    layer1: Vec<Box<BoxedQuad>>,
    layer2: Vec<Box<BoxedQuad>>,
}

impl std::fmt::Debug for HeapQuadAllocator {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fmt.debug_struct("HeapQuadAllocator").finish()
    }
}

impl HeapQuadAllocator {
    pub fn apply_to(&self, other: &mut TripleLayerQuadAllocator) -> anyhow::Result<()> {
        let start = std::time::Instant::now();
        for (layer_num, quads) in [(0, &self.layer0), (1, &self.layer1), (2, &self.layer2)] {
            for quad in quads {
                other.extend_with(layer_num, &quad.to_vertices());
            }
        }
        metrics::histogram!("quad_buffer_apply", start.elapsed());
        Ok(())
    }
}

impl TripleLayerQuadAllocatorTrait for HeapQuadAllocator {
    fn allocate(&mut self, layer_num: usize) -> anyhow::Result<QuadImpl> {
        let quads = match layer_num {
            0 => &mut self.layer0,
            1 => &mut self.layer1,
            2 => &mut self.layer2,
            _ => unreachable!(),
        };

        quads.push(Box::new(BoxedQuad::default()));

        let quad = quads.last_mut().unwrap();
        Ok(QuadImpl::Boxed(quad))
    }

    fn extend_with(&mut self, layer_num: usize, vertices: &[Vertex]) {
        if vertices.is_empty() {
            return;
        }

        let dest_quads = match layer_num {
            0 => &mut self.layer0,
            1 => &mut self.layer1,
            2 => &mut self.layer2,
            _ => unreachable!(),
        };

        // This is logically equivalent to
        // https://doc.rust-lang.org/std/primitive.slice.html#method.as_chunks_unchecked
        // which is currently nightly-only
        assert_eq!(vertices.len() % VERTICES_PER_CELL, 0);
        let src_quads: &[[Vertex; VERTICES_PER_CELL]] =
            unsafe { std::slice::from_raw_parts(vertices.as_ptr().cast(), vertices.len() / 4) };

        for quad in src_quads {
            dest_quads.push(Box::new(BoxedQuad::from_vertices(quad)));
        }
    }
}

pub enum TripleLayerQuadAllocator<'a> {
    Gpu(BorrowedLayers),
    Heap(&'a mut HeapQuadAllocator),
}

impl<'a> TripleLayerQuadAllocatorTrait for TripleLayerQuadAllocator<'a> {
    fn allocate(&mut self, layer_num: usize) -> anyhow::Result<QuadImpl> {
        match self {
            Self::Gpu(b) => b.allocate(layer_num),
            Self::Heap(h) => h.allocate(layer_num),
        }
    }

    fn extend_with(&mut self, layer_num: usize, vertices: &[Vertex]) {
        match self {
            Self::Gpu(b) => b.extend_with(layer_num, vertices),
            Self::Heap(h) => h.extend_with(layer_num, vertices),
        }
    }
}

#[cfg(test)]
#[test]
fn size() {
    assert_eq!(std::mem::size_of::<Vertex>() * VERTICES_PER_CELL, 272);
    assert_eq!(std::mem::size_of::<BoxedQuad>(), 84);
}
