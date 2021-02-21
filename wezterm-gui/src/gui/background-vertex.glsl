// This is the window background shader.
// It places the background image in the full viewport.

// Note: vertex-common.glsl is automatically prepended!

void main() {
  pass_through_vertex();

  if (o_has_color == 2.0) {
    // Background image takes up its full coordinates
    gl_Position = projection * vec4(position, 0.0, 1.0);
  } else {
    // Nothing else should render on the background layer
    gl_Position = off_screen();
  }
}
