// This file is automatically prepended to the various
// vertex.glsl files.

precision highp float;

in vec2 position;
in vec2 adjust;
in vec2 tex;
in vec2 underline;
in vec4 bg_color;
in vec4 fg_color;
in vec4 underline_color;
in float has_color;
in vec2 cursor;
in vec4 cursor_color;
in vec3 hsv;

uniform mat4 projection;

out float o_has_color;
out vec2 o_cursor;
out vec2 o_tex;
out vec2 o_underline;
out vec3 o_hsv;
out vec4 o_bg_color;
out vec4 o_cursor_color;
out vec4 o_fg_color;
out vec4 o_underline_color;

void pass_through_vertex() {
  o_tex = tex;
  o_has_color = has_color;
  o_fg_color = fg_color;
  o_bg_color = bg_color;
  o_underline = underline;
  o_underline_color = underline_color;
  o_cursor = cursor;
  o_cursor_color = cursor_color;
  o_hsv = hsv;
}

// Returns a position that is outside of the viewport,
// such that this vertex effectively won't contribute
// the scene being rendered.
// There may be a better way to do this.
vec4 off_screen() {
  return vec4(100.0, 100.0, 100.0, 100.0);
}

