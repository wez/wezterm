// This is the Glyph vertex shader.
// It is responsible for placing the glyph images in the
// correct place on screen.

// Note: vertex-common.glsl is automatically prepended!

void main() {
  pass_through_vertex();

  if (o_has_color == 2.0) {
    // If we're the background image and we're not rendering
    // the background layer, then move this off screen
    gl_Position = off_screen();
  } else {
    // Use only the adjusted cell position to render the glyph
    gl_Position = projection * vec4(position + adjust, 0.0, 1.0);
  }
}
