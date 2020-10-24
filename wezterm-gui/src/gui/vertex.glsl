precision highp float;

attribute vec2 position;
attribute vec2 adjust;
attribute vec2 tex;
attribute vec2 underline;
attribute vec4 bg_color;
attribute vec4 fg_color;
attribute float has_color;
attribute vec2 cursor;
attribute vec4 cursor_color;
attribute vec3 hsv;

uniform mat4 projection;
uniform bool window_bg_layer;
uniform bool bg_and_line_layer;
uniform bool has_background_image;

varying float o_has_color;
varying vec2 o_cursor;
varying vec2 o_tex;
varying vec2 o_underline;
varying vec3 o_hsv;
varying vec4 o_bg_color;
varying vec4 o_cursor_color;
varying vec4 o_fg_color;

// Returns a position that is outside of the viewport,
// such that this vertex effectively won't contribute
// the scene being rendered.
// There may be a better way to do this.
vec4 off_screen() {
  return vec4(100.0, 100.0, 100.0, 100.0);
}

void main() {
    o_tex = tex;
    o_has_color = has_color;
    o_fg_color = fg_color;
    o_bg_color = bg_color;
    o_underline = underline;
    o_cursor = cursor;
    o_cursor_color = cursor_color;
    o_hsv = hsv;

    if (window_bg_layer) {
      if (o_has_color == 2.0) {
        // Background image takes up its full coordinates
        gl_Position = projection * vec4(position, 0.0, 1.0);
      } else {
        // Nothing else should render on the background layer
        gl_Position = off_screen();
      }
    } else if (o_has_color == 2.0) {
      // If we're the background image and we're not rendering
      // the background layer, then move this off screen
      gl_Position = off_screen();
    } else if (bg_and_line_layer) {
      // Want to fill the whole cell when painting backgrounds
      gl_Position = projection * vec4(position, 0.0, 1.0);
    } else {
      // Use only the adjusted cell position to render the glyph
      gl_Position = projection * vec4(position + adjust, 0.0, 1.0);
    }
}
