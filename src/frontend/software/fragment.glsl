#version 330
precision mediump float;

in vec2 o_tex;
in vec4 o_fg_color;
in vec4 o_bg_color;
in float o_has_color;

uniform mat4 projection;
uniform bool bg_and_line_layer;
uniform sampler2D glyph_tex;

out vec4 color;

void main() {
  if (bg_and_line_layer) {
    color = o_bg_color;
  } else {
    color = texture(glyph_tex, o_tex);
    if (o_has_color == 0.0) {
      // if it's not a color emoji, tint with the fg_color
      color.rgb = o_fg_color.rgb;
    }
  }
}
