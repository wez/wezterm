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

vec4 from_linear(vec4 v) {
  return pow(v, vec4(2.2));
}

vec4 to_linear(vec4 v) {
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
  return color;
//  return to_linear(color);
}

void main() {
  if (o_has_color == 3.0) {
    // Solid color block
    color = o_fg_color;
    colorMask = vec4(1.0, 1.0, 1.0, 1.0);
  } else if (o_has_color == 2.0) {
    // The window background attachment
    color = sample_texture(atlas_linear_sampler, o_tex);
    // Apply window_background_image_opacity to the background image
    colorMask = o_fg_color.aaaa;
  } else {
    color = sample_texture(atlas_nearest_sampler, o_tex);
    if (o_has_color == 0.0) {
      // if it's not a color emoji it will be grayscale
      // and we need to tint with the fg_color
      colorMask = color;
      color = o_fg_color;
      color = apply_hsv(color, foreground_text_hsb);
    } else {
      colorMask = color.aaaa;
    }
  }

  color = apply_hsv(color, o_hsv);
}
