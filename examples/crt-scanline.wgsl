// CRT / Scanline post-processing shader for wezterm.
//
// Designed to be subtle enough for daily use while giving the terminal
// a retro CRT feel. All struct/binding declarations are auto-prepended
// by the wezterm post-processing pipeline -- this file only contains
// helper functions and the required fs_postprocess entry point.

// ---------------------------------------------------------------
// Constants -- tweak these to taste
// ---------------------------------------------------------------

// Barrel distortion strength. Higher = more curvature.
const BARREL_STRENGTH: f32 = 0.04;

// Scanline intensity (0 = no scanlines, 1 = fully black lines).
const SCANLINE_INTENSITY: f32 = 0.08;
// How fast scanlines scroll (pixels per second in UV space).
const SCANLINE_SCROLL_SPEED: f32 = 0.02;
// Scanline frequency -- higher = thinner, more frequent lines.
const SCANLINE_FREQUENCY: f32 = 800.0;

// Vignette strength. Higher = darker edges.
const VIGNETTE_STRENGTH: f32 = 0.25;
// Vignette softness. Higher = wider bright center.
const VIGNETTE_SOFTNESS: f32 = 0.45;

// Chromatic aberration offset in UV space. Keep very small.
const CHROMA_OFFSET: f32 = 0.0008;

// Phosphor glow / bloom radius in UV space.
const GLOW_RADIUS: f32 = 0.001;
// How much the glow adds to the original image.
const GLOW_STRENGTH: f32 = 0.12;

// ---------------------------------------------------------------
// Barrel distortion
// ---------------------------------------------------------------
// Simulates the curved glass of a CRT by warping UVs outward
// from the center. Pixels near the edges are pushed further out,
// creating the characteristic pillow/barrel shape.
fn barrel_distort(uv: vec2<f32>) -> vec2<f32> {
    // Re-center so (0,0) is the middle of the screen.
    let centered = uv - 0.5;
    let r2 = dot(centered, centered);
    // Apply radial distortion proportional to squared distance.
    let distorted = centered * (1.0 + BARREL_STRENGTH * r2);
    return distorted + 0.5;
}

// ---------------------------------------------------------------
// Vignette
// ---------------------------------------------------------------
// Darkens pixels toward the screen edges, mimicking the uneven
// brightness of a real CRT where the electron beam is weaker at
// the periphery.
fn vignette(uv: vec2<f32>) -> f32 {
    let centered = uv - 0.5;
    let dist = length(centered);
    return smoothstep(VIGNETTE_SOFTNESS, VIGNETTE_SOFTNESS - VIGNETTE_STRENGTH, dist);
}

// ---------------------------------------------------------------
// Scanlines
// ---------------------------------------------------------------
// Produces horizontal darkening bands that slowly scroll downward,
// imitating the visible raster lines of a CRT phosphor screen.
fn scanline(uv: vec2<f32>, time: f32) -> f32 {
    // Use the vertical position (in pixel-ish units) plus a slow
    // time-based offset so the lines drift gently downward.
    let y = uv.y * SCANLINE_FREQUENCY + time * SCANLINE_SCROLL_SPEED * SCANLINE_FREQUENCY;
    // sin produces [-1,1]; we remap so the darkest point reaches
    // SCANLINE_INTENSITY darkness and the brightest is 1.0.
    let line = 1.0 - SCANLINE_INTENSITY * (0.5 + 0.5 * sin(y * 3.14159265));
    return line;
}

// ---------------------------------------------------------------
// Phosphor glow (cheap bloom approximation)
// ---------------------------------------------------------------
// Averages a small cross-shaped neighbourhood around the pixel to
// simulate the soft phosphor glow / bloom of a CRT. Keeps the
// tap count low (5 taps) so the shader stays lightweight.
fn phosphor_glow(uv: vec2<f32>) -> vec4<f32> {
    let center = textureSample(screen_texture, screen_sampler, uv);
    let left   = textureSample(screen_texture, screen_sampler, uv + vec2<f32>(-GLOW_RADIUS, 0.0));
    let right  = textureSample(screen_texture, screen_sampler, uv + vec2<f32>( GLOW_RADIUS, 0.0));
    let up     = textureSample(screen_texture, screen_sampler, uv + vec2<f32>(0.0, -GLOW_RADIUS));
    let down   = textureSample(screen_texture, screen_sampler, uv + vec2<f32>(0.0,  GLOW_RADIUS));
    let blur = (center + left + right + up + down) / 5.0;
    return mix(center, blur, GLOW_STRENGTH);
}

// ---------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------
@fragment
fn fs_postprocess(in: VertexOutput) -> @location(0) vec4<f32> {
    // --- Barrel distortion ---
    // Warp UVs to simulate CRT screen curvature.
    let uv = barrel_distort(in.uv);

    // Clamp UVs so texture sampling stays in-bounds. We sample
    // *before* the out-of-bounds check because WGSL requires all
    // textureSample calls to be in uniform control flow (no
    // early-return before them).
    let safe_uv = clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0));

    // --- Chromatic aberration ---
    // Offset each colour channel slightly along the horizontal axis
    // to mimic imperfect convergence of the CRT's RGB electron guns.
    let r = textureSample(screen_texture, screen_sampler, safe_uv + vec2<f32>( CHROMA_OFFSET, 0.0)).r;
    let g = textureSample(screen_texture, screen_sampler, safe_uv).g;
    let b = textureSample(screen_texture, screen_sampler, safe_uv + vec2<f32>(-CHROMA_OFFSET, 0.0)).b;

    // --- Phosphor glow ---
    // Blend in a soft neighbourhood average to simulate bloom.
    let glow = phosphor_glow(safe_uv);

    // If the distorted UV falls outside [0,1] we are past the
    // curved screen edge -- render black.
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }

    var color = vec3<f32>(r, g, b);
    color = mix(color, glow.rgb, GLOW_STRENGTH);

    // --- Scanlines ---
    // Multiply by the scanline mask to add horizontal dark bands.
    color *= scanline(uv, pp.time);

    // --- Vignette ---
    // Darken toward the edges of the screen.
    color *= vignette(uv);

    // Preserve fully opaque alpha so compositing is unaffected.
    return vec4<f32>(color, 1.0);
}
