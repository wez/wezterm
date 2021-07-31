// This is the Glyph fragment shader.
// It is responsible for laying down the glyph graphics on top of the other layers.

// Note: fragment-common.glsl is automatically prepended!

uniform sampler2D atlas_nearest_sampler;
uniform sampler2D atlas_linear_sampler;

void main() {
  if (o_has_color == 2.0) {
    // The window background attachment
    color = sample_texture(atlas_linear_sampler, o_tex);
    // Apply window_background_image_opacity to the background image
    color.a = o_fg_color.a;
  } else {
    color = sample_texture(atlas_nearest_sampler, o_tex);
    if (o_has_color == 0.0) {
      // if it's not a color emoji it will be grayscale
      // and we need to tint with the fg_color
      color = colorize2(color, o_fg_color);
      color = apply_hsv(color, foreground_text_hsb);
    }
  }

  color = apply_hsv(color, o_hsv);
}
