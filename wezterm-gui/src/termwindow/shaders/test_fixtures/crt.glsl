// source: https://gist.github.com/qwerasd205/c3da6c610c8ffe17d6d2d3cc7068f17f
// credits: https://github.com/qwerasd205
//==============================================================
//
//    [CRTS] PUBLIC DOMAIN CRT-STYLED SCALAR by Timothy Lottes
//
//    [+] Adapted with alterations for use in Ghostty by Qwerasd.
//    For more information on changes, see comment below license.
//
//==============================================================
//
//      LICENSE = UNLICENSE (aka PUBLIC DOMAIN)
//
//--------------------------------------------------------------
// This is free and unencumbered software released into the
// public domain.
//--------------------------------------------------------------
// Anyone is free to copy, modify, publish, use, compile, sell,
// or distribute this software, either in source code form or as
// a compiled binary, for any purpose, commercial or
// non-commercial, and by any means.
//--------------------------------------------------------------
// In jurisdictions that recognize copyright laws, the author or
// authors of this software dedicate any and all copyright
// interest in the software to the public domain. We make this
// dedication for the benefit of the public at large and to the
// detriment of our heirs and successors. We intend this
// dedication to be an overt act of relinquishment in perpetuity
// of all present and future rights to this software under
// copyright law.
//--------------------------------------------------------------
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY
// KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE
// WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR
// PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS BE
// LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN
// AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT
// OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.
//--------------------------------------------------------------
// For more information, please refer to
// <http://unlicense.org/>
//==============================================================

// This shader is a modified version of the excellent
// FixingPixelArtFast by Timothy Lottes on Shadertoy.
//
// The original shader can be found at:
// https://www.shadertoy.com/view/MtSfRK
//
// Modifications have been made to reduce the verbosity,
// and many of the comments have been removed / reworded.
// Additionally, the license has been moved to the top of
// the file, and can be read above. I (Qwerasd) choose to
// release the modified version under the same license.

// The appearance of this shader can be altered
// by adjusting the parameters defined below.

// "Scanlines" per real screen pixel.
// e.g. SCALE 0.5 means each scanline is 2 pixels.
// Recommended values:
//  o High DPI displays: 0.33333333
//  - Low DPI displays:  0.66666666
#define SCALE 0.33333333

// "Tube" warp
#define CRTS_WARP 1

// Darkness of vignette in corners after warping
//  0.0 = completely black
//  1.0 = no vignetting
#define MIN_VIN 0.5

// Try different masks
// #define CRTS_MASK_GRILLE 1
// #define CRTS_MASK_GRILLE_LITE 1
// #define CRTS_MASK_NONE 1
#define CRTS_MASK_SHADOW 1

// Scanline thinness
//  0.50 = fused scanlines
//  0.70 = recommended default
//  1.00 = thinner scanlines (too thin)
#define INPUT_THIN 0.75

// Horizonal scan blur
//  -3.0 = pixely
//  -2.5 = default
//  -2.0 = smooth
//  -1.0 = too blurry
#define INPUT_BLUR -2.75

// Shadow mask effect, ranges from,
//  0.25 = large amount of mask (not recommended, too dark)
//  0.50 = recommended default
//  1.00 = no shadow mask
#define INPUT_MASK 0.65

float FromSrgb1(float c) {
  return (c <= 0.04045) ? c * (1.0 / 12.92) :
  pow(c * (1.0 / 1.055) + (0.055 / 1.055), 2.4);
}
vec3 FromSrgb(vec3 c) {
  return vec3(
    FromSrgb1(c.r), FromSrgb1(c.g), FromSrgb1(c.b));
}

vec3 CrtsFetch(vec2 uv) {
  return FromSrgb(texture(iChannel0, uv.xy).rgb);
}

#define CrtsRcpF1(x) (1.0/(x))
#define CrtsSatF1(x) clamp((x),0.0,1.0)

float CrtsMax3F1(float a, float b, float c) {
  return max(a, max(b, c));
}

vec2 CrtsTone(
  float thin,
  float mask) {
  #ifdef CRTS_MASK_NONE
  mask = 1.0;
  #endif

  #ifdef CRTS_MASK_GRILLE_LITE
  // Normal R mask is {1.0,mask,mask}
  // LITE   R mask is {mask,1.0,1.0}
  mask = 0.5 + mask * 0.5;
  #endif

  vec2 ret;
  float midOut = 0.18 / ((1.5 - thin) * (0.5 * mask + 0.5));
  float pMidIn = 0.18;
  ret.x = ((-pMidIn) + midOut) / ((1.0 - pMidIn) * midOut);
  ret.y = ((-pMidIn) * midOut + pMidIn) / (midOut * (-pMidIn) + midOut);

  return ret;
}

vec3 CrtsMask(vec2 pos, float dark) {
  #ifdef CRTS_MASK_GRILLE
  vec3 m = vec3(dark, dark, dark);
  float x = fract(pos.x * (1.0 / 3.0));
  if (x < (1.0 / 3.0)) m.r = 1.0;
  else if (x < (2.0 / 3.0)) m.g = 1.0;
  else m.b = 1.0;
  return m;
  #endif

  #ifdef CRTS_MASK_GRILLE_LITE
  vec3 m = vec3(1.0, 1.0, 1.0);
  float x = fract(pos.x * (1.0 / 3.0));
  if (x < (1.0 / 3.0)) m.r = dark;
  else if (x < (2.0 / 3.0)) m.g = dark;
  else m.b = dark;
  return m;
  #endif

  #ifdef CRTS_MASK_NONE
  return vec3(1.0, 1.0, 1.0);
  #endif

  #ifdef CRTS_MASK_SHADOW
  pos.x += pos.y * 3.0;
  vec3 m = vec3(dark, dark, dark);
  float x = fract(pos.x * (1.0 / 6.0));
  if (x < (1.0 / 3.0)) m.r = 1.0;
  else if (x < (2.0 / 3.0)) m.g = 1.0;
  else m.b = 1.0;
  return m;
  #endif
}

vec3 CrtsFilter(
  vec2 ipos,
  vec2 inputSizeDivOutputSize,
  vec2 halfInputSize,
  vec2 rcpInputSize,
  vec2 rcpOutputSize,
  vec2 twoDivOutputSize,
  float inputHeight,
  vec2 warp,
  float thin,
  float blur,
  float mask,
  vec2 tone
) {
  // Optional apply warp
  vec2 pos;
  #ifdef CRTS_WARP
  // Convert to {-1 to 1} range
  pos = ipos * twoDivOutputSize - vec2(1.0, 1.0);

  // Distort pushes image outside {-1 to 1} range
  pos *= vec2(
      1.0 + (pos.y * pos.y) * warp.x,
      1.0 + (pos.x * pos.x) * warp.y);

  // TODO: Vignette needs optimization
  float vin = 1.0 - (
      (1.0 - CrtsSatF1(pos.x * pos.x)) * (1.0 - CrtsSatF1(pos.y * pos.y)));
  vin = CrtsSatF1((-vin) * inputHeight + inputHeight);

  // Leave in {0 to inputSize}
  pos = pos * halfInputSize + halfInputSize;
  #else
  pos = ipos * inputSizeDivOutputSize;
  #endif

  // Snap to center of first scanline
  float y0 = floor(pos.y - 0.5) + 0.5;
  // Snap to center of one of four pixels
  float x0 = floor(pos.x - 1.5) + 0.5;

  // Inital UV position
  vec2 p = vec2(x0 * rcpInputSize.x, y0 * rcpInputSize.y);
  // Fetch 4 nearest texels from 2 nearest scanlines
  vec3 colA0 = CrtsFetch(p);
  p.x += rcpInputSize.x;
  vec3 colA1 = CrtsFetch(p);
  p.x += rcpInputSize.x;
  vec3 colA2 = CrtsFetch(p);
  p.x += rcpInputSize.x;
  vec3 colA3 = CrtsFetch(p);
  p.y += rcpInputSize.y;
  vec3 colB3 = CrtsFetch(p);
  p.x -= rcpInputSize.x;
  vec3 colB2 = CrtsFetch(p);
  p.x -= rcpInputSize.x;
  vec3 colB1 = CrtsFetch(p);
  p.x -= rcpInputSize.x;
  vec3 colB0 = CrtsFetch(p);

  // Vertical filter
  // Scanline intensity is using sine wave
  // Easy filter window and integral used later in exposure
  float off = pos.y - y0;
  float pi2 = 6.28318530717958;
  float hlf = 0.5;
  float scanA = cos(min(0.5, off * thin) * pi2) * hlf + hlf;
  float scanB = cos(min(0.5, (-off) * thin + thin) * pi2) * hlf + hlf;

  // Horizontal kernel is simple gaussian filter
  float off0 = pos.x - x0;
  float off1 = off0 - 1.0;
  float off2 = off0 - 2.0;
  float off3 = off0 - 3.0;
  float pix0 = exp2(blur * off0 * off0);
  float pix1 = exp2(blur * off1 * off1);
  float pix2 = exp2(blur * off2 * off2);
  float pix3 = exp2(blur * off3 * off3);
  float pixT = CrtsRcpF1(pix0 + pix1 + pix2 + pix3);

  #ifdef CRTS_WARP
  // Get rid of wrong pixels on edge
  pixT *= max(MIN_VIN, vin);
  #endif

  scanA *= pixT;
  scanB *= pixT;

  // Apply horizontal and vertical filters
  vec3 color =
    (colA0 * pix0 + colA1 * pix1 + colA2 * pix2 + colA3 * pix3) * scanA +
      (colB0 * pix0 + colB1 * pix1 + colB2 * pix2 + colB3 * pix3) * scanB;

  // Apply phosphor mask
  color *= CrtsMask(ipos, mask);

  // Tonal control, start by protecting from /0
  float peak = max(1.0 / (256.0 * 65536.0),
      CrtsMax3F1(color.r, color.g, color.b));
  // Compute the ratios of {R,G,B}
  vec3 ratio = color * CrtsRcpF1(peak);
  // Apply tonal curve to peak value
  peak = peak * CrtsRcpF1(peak * tone.x + tone.y);
  // Reconstruct color
  return ratio * peak;
}

float ToSrgb1(float c) {
  return (c < 0.0031308 ? c * 12.92 : 1.055 * pow(c, 0.41666) - 0.055);
}
vec3 ToSrgb(vec3 c) {
  return vec3(
    ToSrgb1(c.r), ToSrgb1(c.g), ToSrgb1(c.b));
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  float aspect = iResolution.x / iResolution.y;
  fragColor.rgb = CrtsFilter(
      fragCoord.xy,
      vec2(1.0),
      iResolution.xy * SCALE * 0.5,
      1.0 / (iResolution.xy * SCALE),
      1.0 / iResolution.xy,
      2.0 / iResolution.xy,
      iResolution.y,
      vec2(1.0 / (50.0 * aspect), 1.0 / 50.0),
      INPUT_THIN,
      INPUT_BLUR,
      INPUT_MASK,
      CrtsTone(INPUT_THIN, INPUT_MASK)
    );

  // Linear to SRGB for output.
  fragColor = vec4(ToSrgb(fragColor.rgb), 1.0);
}
