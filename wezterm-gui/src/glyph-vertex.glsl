// This is the Glyph vertex shader.
// It is responsible for placing the glyph images in the
// correct place on screen.

// Note: vertex-common.glsl is automatically prepended!

void main() {
  pass_through_vertex();

  // Use only the adjusted cell position to render the glyph
  gl_Position = projection * vec4(position + adjust, 0.0, 1.0);
}
