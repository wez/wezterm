// Clippy hates the implement_vertex macro and won't let me scope
// this warning to its use
#![allow(clippy::unneeded_field_pattern)]

use ::window::bitmaps::TextureRect;
use ::window::color::LinearRgba;

/// Each cell is composed of two triangles built from 4 vertices.
/// The buffer is organized row by row.
pub const VERTICES_PER_CELL: usize = 4;
pub const V_TOP_LEFT: usize = 0;
pub const V_TOP_RIGHT: usize = 1;
pub const V_BOT_LEFT: usize = 2;
pub const V_BOT_RIGHT: usize = 3;

#[derive(Copy, Clone, Default)]
pub struct Vertex {
    // Physical position of the corner of the character cell
    pub position: (f32, f32),
    // bearing offset within the cell
    pub adjust: (f32, f32),
    // glyph texture
    pub tex: (f32, f32),
    pub fg_color: (f32, f32, f32, f32),
    pub bg_color: (f32, f32, f32, f32),
    pub hsv: (f32, f32, f32),
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
}
::window::glium::implement_vertex!(
    Vertex, position, adjust, tex, fg_color, bg_color, hsv, has_color
);

/// A helper for updating the 4 vertices that compose a glyph cell
pub struct Quad<'a> {
    pub(crate) vert: &'a mut [Vertex],
}

impl<'a> Quad<'a> {
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

    /// Mark this quad as a background image.
    /// Mutually exclusive with set_has_color.
    pub fn set_is_background_image(&mut self) {
        for v in self.vert.iter_mut() {
            v.has_color = 2.0;
        }
    }

    pub fn set_is_background(&mut self) {
        for v in self.vert.iter_mut() {
            v.has_color = 3.0;
        }
    }

    pub fn set_fg_color(&mut self, color: LinearRgba) {
        for v in self.vert.iter_mut() {
            v.fg_color = color.tuple();
        }
    }

    pub fn set_bg_color(&mut self, color: LinearRgba) {
        for v in self.vert.iter_mut() {
            v.bg_color = color.tuple();
        }
    }

    pub fn set_hsv(&mut self, hsv: Option<config::HsbTransform>) {
        let s = hsv
            .map(|t| (t.hue, t.saturation, t.brightness))
            .unwrap_or((1., 1., 1.));
        for v in self.vert.iter_mut() {
            v.hsv = s;
        }
    }

    #[allow(unused)]
    pub fn get_position(&self) -> (f32, f32, f32, f32) {
        let top_left = self.vert[V_TOP_LEFT].position;
        let bottom_right = self.vert[V_BOT_RIGHT].position;
        (top_left.0, top_left.1, bottom_right.0, bottom_right.1)
    }

    pub fn set_position(&mut self, left: f32, top: f32, right: f32, bottom: f32) {
        self.vert[V_TOP_LEFT].position = (left, top);
        self.vert[V_TOP_RIGHT].position = (right, top);
        self.vert[V_BOT_LEFT].position = (left, bottom);
        self.vert[V_BOT_RIGHT].position = (right, bottom);
    }
}
