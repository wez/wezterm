// This shader is responsible for coloring the window background.

// Note: fragment-common.glsl is automatically prepended!

uniform sampler2D atlas_linear_sampler;

void main() {
  if (o_has_color == 2.0) {
    // We're the window background image.
    color = sample_texture(atlas_linear_sampler, o_tex);
    // Apply window_background_image_opacity to the background image
    color.a = o_bg_color.a;
  } else if (o_has_color == 3.0) {
    color = o_bg_color;
  } else {
    // Nothing else should render on the background layer
    discard;
  }
  color = apply_hsv(color, o_hsv);
}
