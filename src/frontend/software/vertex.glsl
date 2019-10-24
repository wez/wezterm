#version 330

in vec2 position;
in vec2 adjust;
in vec2 tex;
in vec4 bg_color;
in vec4 fg_color;
in float has_color;

uniform mat4 projection;
uniform bool bg_and_line_layer;

out vec2 o_tex;
out vec4 o_fg_color;
out vec4 o_bg_color;
out float o_has_color;

void main() {
    o_tex = tex;
    o_has_color = has_color;
    o_fg_color = fg_color;
    o_bg_color = bg_color;
    if (bg_and_line_layer) {
      // Want to fill the whole cell when painting backgrounds
      gl_Position = projection * vec4(position, 0.0, 1.0);
    } else {
      // Use only the adjusted cell position to render the glyph
      gl_Position = projection * vec4(position + adjust, 0.0, 1.0);
    }
}
