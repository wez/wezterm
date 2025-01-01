// This is the Glyph vertex shader.
// It is responsible for placing the glyph images in the
// correct place on screen.

precision highp float;

in vec2 position;
in vec2 tex;
in vec4 fg_color;
in float has_color;
in float mix_value;
in vec3 hsv;
in vec4 alt_color;

uniform mat4 projection;

out float o_has_color;
out vec2 o_tex;
out vec3 o_hsv;
out vec4 o_fg_color;
out vec4 o_fg_color_alt;
out float o_fg_color_mix;

void pass_through_vertex() {
  o_tex = tex;
  o_has_color = has_color;
  o_fg_color = fg_color;
  o_fg_color_alt = alt_color;
  o_fg_color_mix = mix_value;
  o_hsv = hsv;
}

void main() {
  pass_through_vertex();

  // Use the adjusted cell position to render the quad
  gl_Position = projection * vec4(position, 0.0, 1.0);
}
