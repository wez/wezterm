// This is the Glyph fragment shader.
// It is responsible for laying down the glyph graphics on top of the other layers.
#extension GL_EXT_blend_func_extended: enable
precision highp float;

in float o_has_color;
in vec2 o_tex;
in vec3 o_hsv;
in vec4 o_fg_color;

// The color + alpha
layout(location=0, index=0) out vec4 color;
// Individual alpha channels for RGBA in color, used for subpixel
// antialiasing blending
layout(location=0, index=1) out vec4 colorMask;

uniform vec3 foreground_text_hsb;
uniform sampler2D atlas_nearest_sampler;
uniform sampler2D atlas_linear_sampler;
uniform bool subpixel_aa;

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

const vec3 unit3 = vec3(1.0);

vec4 apply_hsv(vec4 c, vec3 transform)
{
  if (transform == unit3) {
    return c;
  }
  vec3 hsv = rgb2hsv(c.rgb) * transform;
  return vec4(hsv2rgb(hsv).rgb, c.a);
}

/*
float to_srgb(float x) {
  if (x <= 0.0031308) {
    return 12.92 * x;
  }
  return 1.055 * pow(x, (1.0 / 2.4)) - 0.055;
}

vec3 to_srgb(vec3 c) {
  return vec3(to_srgb(c.r), to_srgb(c.g), to_srgb(c.b));
}

vec4 to_srgb(vec4 v) {
  return vec4(to_srgb(v.rgb), v.a);
}
*/

vec4 to_srgb(vec4 linearRGB)
{
  bvec3 cutoff = lessThan(linearRGB.rgb, vec3(0.0031308));
  vec3 higher = vec3(1.055)*pow(linearRGB.rgb, vec3(1.0/2.4)) - vec3(0.055);
  vec3 lower = linearRGB.rgb * vec3(12.92);

  return vec4(mix(higher, lower, cutoff), linearRGB.a);
}

void main() {
  if (o_has_color == 3.0) {
    // Solid color block
    color = o_fg_color;
    colorMask = vec4(1.0);
  } else if (o_has_color == 2.0) {
    // The window background attachment
    color = texture(atlas_linear_sampler, o_tex);
    // Apply window_background_image_opacity to the background image
    if (subpixel_aa) {
      colorMask = o_fg_color.aaaa;
    } else {
      color.a = o_fg_color.a;
    }
  } else if (o_has_color == 1.0) {
    // the texture is full color info (eg: color emoji glyph)
    color = texture(atlas_nearest_sampler, o_tex);
    // this is the alpha
    colorMask = color.aaaa;
  } else if (o_has_color == 4.0) {
    // Grayscale poly quad for non-aa text render layers
    colorMask = texture(atlas_nearest_sampler, o_tex);
    color = o_fg_color;
    color.a *= colorMask.a;
  } else if (o_has_color == 0.0) {
    // the texture is the alpha channel/color mask
    colorMask = texture(atlas_nearest_sampler, o_tex);
    // and we need to tint with the fg_color
    color = o_fg_color;
    if (!subpixel_aa) {
      color.a = colorMask.a;
    }
    color = apply_hsv(color, foreground_text_hsb);
  }

  color = apply_hsv(color, o_hsv);

  // We MUST output SRGB and tell glium that we do that (outputs_srgb),
  // otherwise something in glium over-gamma-corrects depending on the gl setup.
  color = to_srgb(color);
}
