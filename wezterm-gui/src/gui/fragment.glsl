precision highp float;

in float o_has_color;
in vec2 o_cursor;
in vec2 o_tex;
in vec2 o_underline;
in vec3 o_hsv;
in vec4 o_bg_color;
in vec4 o_cursor_color;
in vec4 o_fg_color;
in vec4 o_underline_color;

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

// Given glyph, the greyscale rgba value computed by freetype,
// and color, the desired color, compute the resultant pixel
// value.
// The freetype glyph is greyscale (R=G=B=A) when font_antialias=Greyscale,
// but holds separate intensity values (alpha) for the R, G and
// B channels when font_antialias=Subpixel, with an approximated A
// value derived from the RGB values.
// In terms of computing the color, we can scale each of the color
// RGB values by the glyph RGB values (which are really intensity).
// To reduce darker fringes, the RGB values are scaled down by A
// so that the overall A value doesn't make them too dark at
// end.
vec4 colorize(vec4 glyph, vec4 color) {
  return vec4(glyph.rgb * color.rgb / glyph.a, glyph.a);
}

// The same thing as colorize above, but carried out by first
// translating the color to HSV and then scaling the V (intensity)
// by the glyph alpha level, and then converting back.
// By manipulating the intensity in HSV colorspace, we more accurately
// model the perceived brightness of the individual RGB channel values
// and the appearance of darker AA fringes is reduced.
// However, because this takes only glyph.a into consideration, it may not
// be "as good" as `colorize` when font_antialias=Subpixel.
// To my eye, colorize_hsv looks better than colorize in both of those AA modes.
vec4 colorize_hsv(vec4 glyph, vec4 color) {
  vec3 hsv = rgb2hsv(color.rgb);
  hsv.b *= glyph.a;
  return vec4(hsv2rgb(hsv) / glyph.a, glyph.a);
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
        // we take the underline color, otherwise we'll leave the color
        // at the background color.
        color = o_underline_color;
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
        if (o_fg_color == o_bg_color) {
          // However, if we're a monochrome glyph and the foreground and
          // background colors are the same, just render a transparent pixel
          // instead; this avoids generating shadowy anti-aliasing artifacts
          // for something that should otherwise be invisible.
          color = vec4(0.0, 0.0, 0.0, 0.0);
          discard;
          return;
        } else {
          color = colorize_hsv(color, o_fg_color);
        }
      }
    }
  }

  color = apply_hsv(color);
}
