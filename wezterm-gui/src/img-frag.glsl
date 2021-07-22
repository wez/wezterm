// This is the per-cell image attachment fragment shader.

// Note: fragment-common.glsl is automatically prepended!

uniform sampler2D atlas_nearest_sampler;

void main() {
  if (o_has_color >= 2.0) {
    // Don't render the background image on anything other than
    // the window_bg_layer.
    discard;
    return;
  }
  color = sample_texture(atlas_nearest_sampler, o_img_tex);
  color = apply_hsv(color, o_hsv);
}
