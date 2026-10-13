// Bloom/Glow Shader for WezTerm
// Adds a soft glow effect to bright areas of the terminal
//
// Usage in wezterm.lua:
//   config.front_end = "WebGpu"
//   config.webgpu_shader = wezterm.config_dir .. "/bloom.wgsl"

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

// Get luminance of a color
fn luminance(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.299, 0.587, 0.114));
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let pixel_size = 1.0 / uniforms.resolution;
    let original = textureSample(input_texture, input_sampler, in.uv).rgb;

    // Large blur kernel for visible glow (wider spread = 4-8 pixels)
    // Center sample
    var blurred = textureSample(input_texture, input_sampler, in.uv).rgb * 0.15;

    // Inner ring (4 pixels away)
    let r1 = 4.0;
    blurred += textureSample(input_texture, input_sampler, in.uv + vec2<f32>(-r1, 0.0) * pixel_size).rgb * 0.1;
    blurred += textureSample(input_texture, input_sampler, in.uv + vec2<f32>(r1, 0.0) * pixel_size).rgb * 0.1;
    blurred += textureSample(input_texture, input_sampler, in.uv + vec2<f32>(0.0, -r1) * pixel_size).rgb * 0.1;
    blurred += textureSample(input_texture, input_sampler, in.uv + vec2<f32>(0.0, r1) * pixel_size).rgb * 0.1;

    // Outer ring (8 pixels away)
    let r2 = 8.0;
    blurred += textureSample(input_texture, input_sampler, in.uv + vec2<f32>(-r2, 0.0) * pixel_size).rgb * 0.06;
    blurred += textureSample(input_texture, input_sampler, in.uv + vec2<f32>(r2, 0.0) * pixel_size).rgb * 0.06;
    blurred += textureSample(input_texture, input_sampler, in.uv + vec2<f32>(0.0, -r2) * pixel_size).rgb * 0.06;
    blurred += textureSample(input_texture, input_sampler, in.uv + vec2<f32>(0.0, r2) * pixel_size).rgb * 0.06;

    // Diagonal samples
    let d = 5.6;  // ~4*sqrt(2)
    blurred += textureSample(input_texture, input_sampler, in.uv + vec2<f32>(-d, -d) * pixel_size).rgb * 0.04;
    blurred += textureSample(input_texture, input_sampler, in.uv + vec2<f32>(d, -d) * pixel_size).rgb * 0.04;
    blurred += textureSample(input_texture, input_sampler, in.uv + vec2<f32>(-d, d) * pixel_size).rgb * 0.04;
    blurred += textureSample(input_texture, input_sampler, in.uv + vec2<f32>(d, d) * pixel_size).rgb * 0.04;

    // Create bloom from bright areas - lower threshold for more visible effect
    let lum = luminance(blurred);
    let bloom_strength = smoothstep(0.1, 0.5, lum);
    let bloom = blurred * bloom_strength * 1.5;

    // Combine original with bloom - stronger effect
    var col = original + bloom * 0.8;

    // Slight saturation boost for that glowy feel
    let gray = dot(col, vec3<f32>(0.299, 0.587, 0.114));
    col = mix(vec3<f32>(gray), col, 1.2);

    return vec4<f32>(col, 1.0);
}
