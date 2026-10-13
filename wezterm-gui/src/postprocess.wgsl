// Post-process pipeline: vertex shader and default passthrough fragment shader.
//
// The vertex shader generates a fullscreen triangle from 3 vertices
// with no index buffer required.
//
// User fragment shaders will have the binding declarations below
// auto-prepended, so they only need to define `fn fs_postprocess`.

struct PostProcessUniform {
    resolution: vec2<f32>,
    time: f32,
    time_delta: f32,
    frame: u32,
    _padding_0: u32,
    _padding_1: u32,
    _padding_2: u32,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var screen_sampler: sampler;

@group(1) @binding(0) var<uniform> pp: PostProcessUniform;

@vertex
fn vs_postprocess(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    // Fullscreen triangle: 3 vertices that cover the entire clip space.
    // vertex 0: (-1, -1)  vertex 1: (3, -1)  vertex 2: (-1, 3)
    var out: VertexOutput;
    let x = f32(i32(vertex_index & 1u) * 4 - 1);
    let y = f32(i32(vertex_index >> 1u) * 4 - 1);
    out.position = vec4<f32>(x, y, 0.0, 1.0);
    // Map clip coords to UV: x [-1,1] -> [0,1], y [-1,1] -> [1,0] (flip Y)
    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    return out;
}

// Default passthrough fragment shader: samples the screen texture unchanged.
@fragment
fn fs_postprocess(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(screen_texture, screen_sampler, in.uv);
}
