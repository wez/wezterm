// Vignette Shader for WezTerm
// Darkens the edges of the screen for a focused/cinematic look

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
    var col = textureSample(input_texture, input_sampler, in.uv).rgb;

    // Vignette - darkens edges, bright in center
    let center = in.uv - 0.5;
    let vignette = 1.0 - dot(center, center) * 1.2;
    col *= vignette;

    return vec4<f32>(col, 1.0);
}
