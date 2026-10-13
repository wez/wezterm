// Retro CRT Shader for WezTerm
// Inspired by 80s aesthetics, Tron, and Blade Runner
//
// Effects included:
// - CRT screen curvature
// - Scanlines
// - Chromatic aberration
// - Vignette
// - Subtle bloom/glow
// - Film grain
// - Color grading (cyan/magenta push)
//
// Usage in wezterm.lua:
//   config.front_end = "WebGpu"
//   config.webgpu_shader = wezterm.config_dir .. "/retro-crt.wgsl"

struct PostProcessUniform {
    resolution: vec2<f32>,
    time: f32,
    _padding: f32,
};

@group(0) @binding(0) var<uniform> uniforms: PostProcessUniform;
@group(0) @binding(1) var input_texture: texture_2d<f32>;
@group(0) @binding(2) var input_sampler: sampler;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    let x = f32(i32(vertex_index & 1u) * 4 - 1);
    let y = f32(i32(vertex_index >> 1u) * 4 - 1);
    out.position = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    return out;
}

// CRT screen curvature
fn crt_curve(uv: vec2<f32>, curvature: f32) -> vec2<f32> {
    let center = uv - 0.5;
    let dist = dot(center, center) * curvature;
    return uv + center * dist;
}

// Chromatic aberration - separates RGB channels
fn chromatic_aberration(uv: vec2<f32>, intensity: f32) -> vec3<f32> {
    let r = textureSample(input_texture, input_sampler, uv + vec2<f32>(intensity, 0.0)).r;
    let g = textureSample(input_texture, input_sampler, uv).g;
    let b = textureSample(input_texture, input_sampler, uv - vec2<f32>(intensity, 0.0)).b;
    return vec3<f32>(r, g, b);
}

// Scanline effect - smoother to reduce moiré
fn scanline(y: f32, intensity: f32) -> f32 {
    let line = sin(y * uniforms.resolution.y * 3.14159 / 3.0);
    return 1.0 - intensity * (0.5 + 0.5 * line);
}

// Vignette - darkens edges (same as vignette.wgsl)
fn vignette(uv: vec2<f32>) -> f32 {
    let center = uv - 0.5;
    return 1.0 - dot(center, center) * 1.2;
}

// Simple pseudo-random for grain
fn rand(co: vec2<f32>) -> f32 {
    return fract(sin(dot(co, vec2<f32>(12.9898, 78.233))) * 43758.5453);
}

// Approximate bloom by sampling nearby pixels
fn bloom(uv: vec2<f32>, intensity: f32) -> vec3<f32> {
    var col = vec3<f32>(0.0);
    let pixel_size = 1.0 / uniforms.resolution;

    for (var x: f32 = -2.0; x <= 2.0; x += 1.0) {
        for (var y: f32 = -2.0; y <= 2.0; y += 1.0) {
            let offset = vec2<f32>(x, y) * pixel_size * 2.0;
            col += textureSample(input_texture, input_sampler, uv + offset).rgb;
        }
    }
    return col / 25.0 * intensity;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Apply CRT curvature (subtle)
    let uv = crt_curve(in.uv, 0.05);

    // Check if outside the curved screen
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }

    // Chromatic aberration
    var col = chromatic_aberration(uv, 0.001);

    // Add bloom for neon glow effect
    let glow = bloom(uv, 1.2);
    col = mix(col, glow, 0.15);

    // Boost cyans and magentas (Tron palette)
    col.r *= 1.0 + 0.08 * col.b;
    col.b *= 1.1;
    col.g *= 0.95 + 0.1 * col.b;

    // Scanlines (subtle)
    col *= scanline(uv.y, 0.15);

    // Subtle horizontal line flicker
    let flicker = 1.0 + 0.01 * sin(uniforms.time * 10.0 + uv.y * 100.0);
    col *= flicker;

    // Vignette
    col *= vignette(uv);

    // Color grade - push toward blue/cyan shadows
    col = mix(col, col * vec3<f32>(0.9, 0.95, 1.1), 0.3);

    // Film grain
    let grain = rand(uv + fract(uniforms.time));
    col += (grain - 0.5) * 0.03;

    return vec4<f32>(col, 1.0);
}
