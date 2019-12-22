// Clippy hates the implement_vertex macro and won't let me scope
// this warning to its use
#![allow(clippy::unneeded_field_pattern)]

use ::window::bitmaps::TextureRect;
use ::window::*;

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
    // underline texture
    pub underline: (f32, f32),
    // cursor texture
    pub cursor: (f32, f32),
    pub cursor_color: (f32, f32, f32, f32),
    pub bg_color: (f32, f32, f32, f32),
    pub fg_color: (f32, f32, f32, f32),
    // "bool can't be an in in the vertex shader"
    pub has_color: f32,
}
::window::glium::implement_vertex!(
    Vertex,
    position,
    adjust,
    tex,
    underline,
    cursor,
    cursor_color,
    bg_color,
    fg_color,
    has_color
);

/// A helper for updating the 4 vertices that compose a glyph cell
pub struct Quad<'a> {
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

    pub fn set_cursor(&mut self, coords: TextureRect) {
        self.vert[V_TOP_LEFT].cursor = (coords.min_x(), coords.min_y());
        self.vert[V_TOP_RIGHT].cursor = (coords.max_x(), coords.min_y());
        self.vert[V_BOT_LEFT].cursor = (coords.min_x(), coords.max_y());
        self.vert[V_BOT_RIGHT].cursor = (coords.max_x(), coords.max_y());
    }

    pub fn set_cursor_color(&mut self, color: Color) {
        let color = color.to_tuple_rgba();
        for v in self.vert.iter_mut() {
            v.cursor_color = color;
        }
    }
}
