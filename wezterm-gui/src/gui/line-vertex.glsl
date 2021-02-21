// This is the underline/strikethrough and text-background color
// shader.  It is responsible for locating the cell boundaries.

// Note: vertex-common.glsl is automatically prepended!

void main() {
  pass_through_vertex();

  if (o_has_color == 2.0) {
    // If we're the background image and we're not rendering
    // the background layer, then move this off screen
    gl_Position = off_screen();
  } else {
    // Want to fill the whole cell when painting backgrounds
    gl_Position = projection * vec4(position, 0.0, 1.0);
  }
}
