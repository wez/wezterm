precision highp float;

in vec2 o_tex;
in vec4 o_fg_color;
in vec4 o_bg_color;
in float o_has_color;
in vec2 o_underline;
in vec2 o_cursor;
in vec4 o_cursor_color;

uniform mat4 projection;
uniform bool window_bg_layer;
uniform bool bg_and_line_layer;
uniform bool has_background_image;
uniform sampler2D glyph_tex;

out vec4 color;

float multiply_one(float src, float dst, float inv_dst_alpha, float inv_src_alpha) {
  return (src * dst) + (src * (inv_dst_alpha)) + (dst * (inv_src_alpha));
}

// Alpha-regulated multiply to colorize the glyph bitmap.
// The texture data is pre-multiplied by the alpha, so we need to divide
// by the alpha after multiplying to avoid having the colors be too dark.
vec4 multiply(vec4 src, vec4 dst) {
  float inv_src_alpha = 1.0 - src.a;
  float inv_dst_alpha = 1.0 - dst.a;

  return vec4(
      multiply_one(src.r, dst.r, inv_dst_alpha, inv_src_alpha) / dst.a,
      multiply_one(src.g, dst.g, inv_dst_alpha, inv_src_alpha) / dst.a,
      multiply_one(src.b, dst.b, inv_dst_alpha, inv_src_alpha) / dst.a,
      dst.a);
}

void main() {
  if (window_bg_layer) {
    if (o_has_color == 2.0) {
      // We're the window background image.
      color = texture(glyph_tex, o_tex);
      // Apply window_background_opacity to the background image
      color.a = o_bg_color.a;
    } else if (!has_background_image) {
      // If there is no background image then take the cell background
      color = o_bg_color;
    } else {
      // Nothing else should render on the background layer
      // color = vec4(0,0,0,0);
      discard;
    }
  } else if (bg_and_line_layer) {
    // Note that o_bg_color is set to transparent if the background
    // color is "default" and there is a window background attachment
    color = o_bg_color;

    // Sample the underline glyph texture for this location.
    // Note that the texture is whitespace in the case where this is
    // no underline or strikethrough.
    vec4 under_color = texture(glyph_tex, o_underline);
    if (under_color.a != 0.0) {
        // if the underline glyph isn't transparent in this position then
        // we take the text fg color, otherwise we'll leave the color
        // at the background color.
        color = o_fg_color;
    }

    // Similar to the above: if the cursor texture isn't transparent
    // in this location, we'll use the cursor color instead of the background.
    // The cursor color overrides any underline color we might have picked
    // in the section above.
    vec4 cursor_outline = texture(glyph_tex, o_cursor);
    if (cursor_outline.a != 0.0) {
      color = o_cursor_color;
    }
  } else {
    if (o_has_color == 2.0) {
      // Don't render the background image on anything other than
      // the window_bg_layer.
      // color = vec4(0,0,0,0);
      discard;
    } else {
      color = texture(glyph_tex, o_tex);
      if (o_has_color == 0.0) {
        // if it's not a color emoji, tint with the fg_color
        color.rgb = o_fg_color.rgb;
      }
    }
  }
}
