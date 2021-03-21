// This is the Glyph fragment shader.
// It is the last stage in drawing, and is responsible
// for laying down the glyph graphics on top of the other layers.

// Note: fragment-common.glsl is automatically prepended!

uniform sampler2D atlas_nearest_sampler;

void main() {
  if (o_has_color >= 2.0) {
    // Don't render the background image on anything other than
    // the window_bg_layer.
    discard;
    return;
  }

  color = texture_sample(atlas_nearest_sampler, o_tex);
  if (o_has_color == 0.0) {
    // if it's not a color emoji it will be grayscale
    // and we need to tint with the fg_color
    if (o_fg_color == o_bg_color) {
      // However, if we're a monochrome glyph and the foreground and
      // background colors are the same, just render a transparent pixel
      // instead; this avoids generating shadowy anti-aliasing artifacts
      // for something that should otherwise be invisible.
      color = vec4(0.0, 0.0, 0.0, 0.0);
      discard;
      return;
    } else {
      color = colorize(color, o_fg_color, o_bg_color);
      color = apply_hsv(color, foreground_text_hsb);
    }
  }

  color = apply_hsv(color, o_hsv);
  if (apply_gamma_to_texture) {
    color = to_linear(color);
  }
}
