// Degraded VHS tape + CRT monitor effect for Ghostty
//
// Author:  Alex Brinsmead (https://github.com/abrinsmead)
// Source:  https://gist.github.com/abrinsmead/be7d2d2209d0dd3097000af369867927
// License: MIT
//
// Copyright (c) 2026 Alex Brinsmead
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
// THE SOFTWARE.
//
// Setup:
//   1. Save to ~/.config/ghostty/shaders/vhs.glsl
//   2. Add to Ghostty config:
//        custom-shader = ~/.config/ghostty/shaders/vhs.glsl
//        custom-shader-animation = true
//   3. Reload with Cmd+Shift+, or restart Ghostty.
//
// ========== TUNABLE PARAMETERS ==========
//
// CRT_CURVE:    Screen curvature. 0.0 = flat, 0.04 = strong bend.
#define CRT_CURVE     0.02
//
// BRIGHTNESS:   Overall brightness multiplier.
#define BRIGHTNESS    1.8
//
// DISTORTION:   Wobble, jitter, tracking band, frame jump, static burst.
//               0.0 = clean signal, 1.0 = normal, 2.0 = heavy.
#define DISTORTION    1.0
//
// TAPE_WEAR:    Grain, noise, dropout, head-switch, warm shift.
//               0.0 = pristine tape, 1.0 = normal, 2.0 = trashed.
#define TAPE_WEAR     1.75
//
// COLOR_BLEED:  Chroma smearing. 0.0 = perfect color, 1.0 = normal, 2.0 = heavy.
#define COLOR_BLEED   2
//
// VIGNETTE:     Corner darkening. 0.0 = none, 0.3 = light, 0.8 = heavy.
#define VIGNETTE      0.35
//
// STATIC:       Persistent white static flecks. 0.0 = none, 0.5 = subtle, 1.0 = normal.
#define STATIC        0.9
//
// =========================================

float hash(vec2 p) {
    p = fract(p * vec2(443.8975, 397.2973));
    p += dot(p, p + 19.19);
    return fract(p.x * p.y);
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord.xy / iResolution.xy;

    // CRT barrel distortion
    vec2 cc = uv - 0.5;
    vec2 wuv = clamp(uv + cc * (dot(cc, cc) * CRT_CURVE), 0.0, 1.0);

    // --- Signal distortion ---

    wuv.x += (sin(wuv.y * 40.0 + iTime * 0.5) * 0.000375
            + sin(wuv.y * 80.0 + iTime * 0.2) * 0.0001875
            + (hash(vec2(floor(fragCoord.y), floor(iTime * 20.0))) - 0.5) * 0.0006) * DISTORTION;

    // Tracking band
    float tracking_y = fract(iTime * 0.04);
    float band = smoothstep(0.05, 0.0, min(abs(wuv.y - tracking_y), 1.0 - abs(wuv.y - tracking_y)));
    float h_offset = band * (hash(vec2(floor(fragCoord.y * 0.5), iTime * 3.0)) - 0.5) * 0.006 * DISTORTION;

    // Static burst (5s window every ~30s)
    float burst_cycle = floor(iTime / 30.0);
    float burst_start = hash(vec2(burst_cycle, 88.0)) * 25.0;
    float burst_time = mod(iTime, 30.0);
    bool burst_active = burst_time > burst_start && burst_time < burst_start + 5.0;

    // Frame jump (burst-only)
    float jump_seed = floor(iTime * 2.0);
    float jump_offset = burst_active ? (hash(vec2(jump_seed, 11.3)) - 0.5) * 0.08 * DISTORTION : 0.0;

    vec2 sample_uv = fract(wuv + vec2(h_offset, jump_offset));

    // --- Color sampling (5 texture fetches) ---

    float aberr = (0.0003 + band * 0.0015) * COLOR_BLEED;
    vec3 color;
    color.r = texture(iChannel0, sample_uv + vec2(aberr, 0.0)).r;
    color.g = texture(iChannel0, sample_uv).g;
    color.b = texture(iChannel0, sample_uv - vec2(aberr, 0.0)).b;

    // Rightward-only neighborhood (VHS only smears right)
    float px = 1.0 / iResolution.x;
    vec3 s_r1 = texture(iChannel0, sample_uv + vec2(px, 0.0)).rgb;
    vec3 s_r2 = texture(iChannel0, sample_uv + vec2(px * 3.0, 0.0)).rgb;

    // Color bleed
    vec3 nb = s_r1 * 0.4 + s_r2 * 0.6;
    float luma = dot(color, vec3(0.299, 0.587, 0.114));
    color = luma + mix(color - luma, nb - dot(nb, vec3(0.299, 0.587, 0.114)), 0.5 * COLOR_BLEED);

    // --- Tape artifacts ---

    if (burst_active) {
        float bm = smoothstep(0.7, 0.95, wuv.y);
        if (bm > 0.0) {
            float t = fract(iTime * 43.0);
            float pull = (hash(vec2(floor(fragCoord.y), t * 100.0)) - 0.5) * 0.02 * DISTORTION;
            vec2 sc = vec2(floor((fragCoord.x + pull * iResolution.x) / 10.0), fragCoord.y);
            color = mix(color, vec3(1.0), step(0.93, fract(sin(dot(sc, vec2(12.9898 + t, 78.233 + t))) * 43758.5453)) * bm * 0.4 * DISTORTION);
        }
    }

    if (jump_offset == 0.0 && wuv.y > 0.97) {
        float hs = smoothstep(0.98, 1.0, wuv.y);
        color += hs * (hash(vec2(fragCoord.x, floor(iTime * 15.0))) * 2.0 - 1.0) * 0.15 * TAPE_WEAR;
    }

    float dr = floor(fragCoord.y / 3.0), dt = floor(iTime * 10.0);
    if (hash(vec2(dr, dt)) > 1.0 - 0.003 * TAPE_WEAR) {
        float dx = hash(vec2(dr, dt + 1.0));
        if (wuv.x > dx && wuv.x < dx + hash(vec2(dr, dt + 2.0)) * 0.15)
            color = mix(color, vec3(1.0), 0.05 * TAPE_WEAR);
    }

    // --- Post-processing ---

    color *= BRIGHTNESS;

    float tg = fract(iTime);
    float cn1 = hash(fragCoord.xy + tg * 100.0) - 0.5;
    float cn2 = hash(fragCoord.xy + tg * 200.0) - 0.5;
    float gl = dot(color, vec3(0.299, 0.587, 0.114));
    color += (color - gl) * vec3(cn1, cn2, -cn1) * 0.15 * TAPE_WEAR;
    color += cn1 * 0.05 * TAPE_WEAR;
    color *= mix(vec3(1.0), vec3(1.03, 1.01, 0.96), TAPE_WEAR);

    vec2 vig = uv * (1.0 - uv);
    color *= mix(1.0, clamp((jump_offset != 0.0) ? vig.x * 4.0 : vig.x * vig.y * 15.0, 0.0, 1.0), VIGNETTE);

    float st = fract(iTime * 20.0);
    color = mix(color, vec3(1.0), step(1.0 - 0.001 * STATIC, hash(vec2(floor(fragCoord.x / 10.0), floor(fragCoord.y / 2.0) + st * 500.0) + st * 77.0)) * 0.15 * STATIC);

    fragColor = vec4(color, 1.0);
}
