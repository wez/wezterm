precision highp float;

in float o_has_color;
in vec2 o_cursor;
in vec2 o_tex;
in vec2 o_underline;
in vec3 o_hsv;
in vec4 o_bg_color;
in vec4 o_cursor_color;
in vec4 o_fg_color;

uniform mat4 projection;
uniform bool window_bg_layer;
uniform bool bg_and_line_layer;
uniform bool has_background_image;

uniform sampler2D atlas_nearest_sampler;
uniform sampler2D atlas_linear_sampler;

out vec4 color;

float multiply_one(float src, float dst, float inv_dst_alpha, float inv_src_alpha) {
  return (src * dst) + (src * (inv_dst_alpha)) + (dst * (inv_src_alpha));
}

// Alpha-regulated multiply to colorize the glyph bitmap.
vec4 multiply(vec4 src, vec4 dst) {
  float inv_src_alpha = 1.0 - src.a;
  float inv_dst_alpha = 1.0 - dst.a;

  return vec4(
      multiply_one(src.r, dst.r, inv_dst_alpha, inv_src_alpha),
      multiply_one(src.g, dst.g, inv_dst_alpha, inv_src_alpha),
      multiply_one(src.b, dst.b, inv_dst_alpha, inv_src_alpha),
      dst.a);
}

vec3 rgb2hsv(vec3 c)
{
    vec4 K = vec4(0.0, -1.0 / 3.0, 2.0 / 3.0, -1.0);
    vec4 p = mix(vec4(c.bg, K.wz), vec4(c.gb, K.xy), step(c.b, c.g));
    vec4 q = mix(vec4(p.xyw, c.r), vec4(c.r, p.yzx), step(p.x, c.r));

    float d = q.x - min(q.w, q.y);
    float e = 1.0e-10;
    return vec3(abs(q.z + (q.w - q.y) / (6.0 * d + e)), d / (q.x + e), q.x);
}

vec3 hsv2rgb(vec3 c)
{
    vec4 K = vec4(1.0, 2.0 / 3.0, 1.0 / 3.0, 3.0);
    vec3 p = abs(fract(c.xxx + K.xyz) * 6.0 - K.www);
    return c.z * mix(K.xxx, clamp(p - K.xxx, 0.0, 1.0), c.y);
}

vec4 apply_hsv(vec4 c)
{
  vec3 hsv = rgb2hsv(c.rgb) * o_hsv;
  return vec4(hsv2rgb(hsv).rgb, c.a);
}

void main() {
  if (window_bg_layer) {
    if (o_has_color == 2.0) {
      // We're the window background image.
      color = texture(atlas_linear_sampler, o_tex);
      // Apply window_background_image_opacity to the background image
      color.a = o_bg_color.a;
    } else if (o_has_color == 3.0) {
      color = o_bg_color;
    } else {
      // Nothing else should render on the background layer
      discard;
    }
  } else if (bg_and_line_layer) {
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
    vec4 under_color = texture(atlas_nearest_sampler, o_underline);
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
    vec4 cursor_outline = texture(atlas_nearest_sampler, o_cursor);
    if (cursor_outline.a != 0.0) {
      color = o_cursor_color;
    }
  } else {
    if (o_has_color >= 2.0) {
      // Don't render the background image on anything other than
      // the window_bg_layer.
      discard;
    } else {
      color = texture(atlas_nearest_sampler, o_tex);
      if (o_has_color == 0.0) {
        // if it's not a color emoji it will be grayscale
        // and we need to tint with the fg_color
        color = multiply(o_fg_color, color);
      }
    }
  }

  color = apply_hsv(color);
}
