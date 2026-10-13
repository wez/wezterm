// Digital Rain Shader for WezTerm
// Matrix-style animated digital rain effect overlaid on terminal
//
// Usage in wezterm.lua:
//   config.front_end = "WebGpu"
//   config.webgpu_shader = wezterm.config_dir .. "/digital-rain.wgsl"

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

// Pseudo-random function
fn rand(co: vec2<f32>) -> f32 {
    return fract(sin(dot(co, vec2<f32>(12.9898, 78.233))) * 43758.5453);
}

// Create a single rain drop column
fn rain_drop(uv: vec2<f32>, col_id: f32, speed: f32) -> f32 {
    let time_offset = rand(vec2<f32>(col_id, 0.0)) * 10.0;
    let drop_length = 0.1 + rand(vec2<f32>(col_id, 1.0)) * 0.15;

    // Position of drop head (moving down)
    let head_y = fract((uniforms.time * speed + time_offset) * 0.3);

    // Distance from drop head
    let dist_from_head = head_y - uv.y;

    // Create trail effect (bright at head, fading behind)
    if (dist_from_head > 0.0 && dist_from_head < drop_length) {
        return (1.0 - dist_from_head / drop_length) * 0.3;
    }

    return 0.0;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var col = textureSample(input_texture, input_sampler, in.uv).rgb;

    // Create rain effect
    let num_columns = 80.0;
    let col_width = 1.0 / num_columns;
    let col_id = floor(in.uv.x * num_columns);

    // Multiple rain layers at different speeds
    var rain = 0.0;
    rain += rain_drop(in.uv, col_id, 1.0) * step(0.7, rand(vec2<f32>(col_id, 2.0)));
    rain += rain_drop(in.uv, col_id + 100.0, 0.7) * step(0.8, rand(vec2<f32>(col_id, 3.0)));
    rain += rain_drop(in.uv, col_id + 200.0, 1.3) * step(0.75, rand(vec2<f32>(col_id, 4.0)));

    // Add green tint from rain
    let rain_color = vec3<f32>(0.0, rain, rain * 0.3);

    // Slight green tint to the whole image
    col = col * vec3<f32>(0.9, 1.0, 0.9);

    // Combine with rain overlay
    col = col + rain_color;

    // Subtle scanlines
    let scanline = sin(in.uv.y * uniforms.resolution.y * 3.14159) * 0.5 + 0.5;
    col *= 0.95 + 0.05 * scanline;

    return vec4<f32>(col, 1.0);
}
