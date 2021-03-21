// This shader is responsible for coloring the underline and
// glyph background graphics.

// Note: fragment-common.glsl is automatically prepended!

uniform sampler2D atlas_nearest_sampler;

void main() {
  if (o_has_color >= 2.0) {
    // Don't render the background image on anything other than
    // the window_bg_layer.
    discard;
    return;
  }

  // Note that o_bg_color is set to transparent if the background
  // color is "default" and there is a window background attachment
  color = o_bg_color;

  // Sample the underline glyph texture for this location.
  // Note that the texture is whitespace in the case where this is
  // no underline or strikethrough.
  vec4 under_color = texture_sample(atlas_nearest_sampler, o_underline);
  if (under_color.a != 0.0) {
    // if the underline glyph isn't transparent in this position then
    // we take the underline color, otherwise we'll leave the color
    // at the background color.
    color = o_underline_color;
  }

  // Similar to the above: if the cursor texture isn't transparent
  // in this location, we'll use the cursor color instead of the background.
  // The cursor color overrides any underline color we might have picked
  // in the section above.
  vec4 cursor_outline = texture_sample(atlas_nearest_sampler, o_cursor);
  if (cursor_outline.a != 0.0) {
    color = o_cursor_color;
  }

  color = apply_hsv(color, o_hsv);
  if (apply_gamma_to_texture) {
    color = to_linear(color);
  }
}
