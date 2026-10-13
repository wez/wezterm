// Scanlines Shader for WezTerm
// CRT-style horizontal scanlines effect
//
// Usage in wezterm.lua:
//   config.front_end = "WebGpu"
//   config.webgpu_shader = wezterm.config_dir .. "/scanlines.wgsl"

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

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Slight RGB separation for CRT feel (chromatic aberration)
    let offset = 0.0015;
    let r = textureSample(input_texture, input_sampler, in.uv + vec2<f32>(offset, 0.0)).r;
    let g = textureSample(input_texture, input_sampler, in.uv).g;
    let b = textureSample(input_texture, input_sampler, in.uv - vec2<f32>(offset, 0.0)).b;
    var col = vec3<f32>(r, g, b);

    // Calculate scanline effect - every 3 pixels
    let y_pixel = in.uv.y * uniforms.resolution.y;
    let scanline_period = 3.0;
    let scanline = sin(y_pixel * 3.14159 / scanline_period);

    // Smooth scanlines: dark lines ~60% brightness, bright lines 100%
    col *= 0.6 + 0.4 * (0.5 + 0.5 * scanline);

    // Subtle vignette effect
    let center = in.uv - 0.5;
    let vignette = 1.0 - dot(center, center) * 0.5;
    col *= vignette;

    // Brightness boost to compensate for darkening
    col *= 1.15;

    return vec4<f32>(col, 1.0);
}
