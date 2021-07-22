// This is the image vertex shader.
// It is responsible for placing per-cell attached images in the
// correct place on screen.

// Note: vertex-common.glsl is automatically prepended!

void main() {
  pass_through_vertex();
  if (o_has_color == 2.0) {
    // If we're the background image and we're not rendering
    // the background layer, then move this off screen
    gl_Position = off_screen();
  } else {
    gl_Position = projection * vec4(position, 0.0, 1.0);
  }
}
