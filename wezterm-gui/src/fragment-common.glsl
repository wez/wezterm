// This file is automatically prepended to the various -frag shaders.

precision highp float;

in float o_has_color;
in vec2 o_cursor;
in vec2 o_tex;
in vec2 o_img_tex;
in vec2 o_underline;
in vec3 o_hsv;
in vec4 o_bg_color;
in vec4 o_cursor_color;
in vec4 o_fg_color;
in vec4 o_underline_color;

out vec4 color;

uniform vec3 foreground_text_hsb;

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

const vec3 unit3 = vec3(1.0, 1.0, 1.0);

vec4 apply_hsv(vec4 c, vec3 transform)
{
  if (transform == unit3) {
    return c;
  }
  vec3 hsv = rgb2hsv(c.rgb) * transform;
  return vec4(hsv2rgb(hsv).rgb, c.a);
}

// Given glyph, the greyscale rgba value computed by freetype,
// and color, the desired color, compute the resultant pixel
// value for rendering over the top of the given background
// color.
//
// The freetype glyph is greyscale (R=G=B=A) when font_antialias=Greyscale,
// where each channel holds the brightness of the pixel.
// It holds separate intensity values for the R, G and B channels when
// subpixel anti-aliasing is in use, with an approximated A value
// derived from the R, G, B values.
//
// In sub-pixel mode we don't want to look at glyph.a as we effective
// have per-channel alpha.  In greyscale mode, glyph.a is the same
// as the other channels, so this routine ignores glyph.a when
// computing the blend, but does include that value for the returned
// alpha value.
//
// See also: https://www.puredevsoftware.com/blog/2019/01/22/sub-pixel-gamma-correct-font-rendering/
vec4 colorize(vec4 glyph, vec4 color, vec4 background) {
  float r = glyph.r * color.r + (1.0 - glyph.r) * background.r;
  float g = glyph.g * color.g + (1.0 - glyph.g) * background.g;
  float b = glyph.b * color.b + (1.0 - glyph.b) * background.b;

  return vec4(r, g, b, glyph.a);
//  return vec4(glyph.rgb * color.rgb, glyph.a);
}

vec4 from_linear(vec4 v) {
  return pow(v, vec4(2.2));
}

vec4 to_gamma(vec4 v) {
  return pow(v, vec4(1.0/2.2));
}

// For reasons that I haven't been able to figure out, we need
// to gamma correct the data that we read from the textures that
// are supplied to OpenGL, otherwise they appear too dark.
// AFAICT, I've done what I thought were all of the right things
// (but are perhaps only some of the right things) to tell OpenGL/EGL
// that everything is already SRGB, so this function should really
// just be a call to `texture` and not do the gamma conversion.
vec4 sample_texture(sampler2D s, vec2 coords) {
  vec4 color = texture(s, coords);
  return to_gamma(color);
}
