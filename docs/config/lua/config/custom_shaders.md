---
tags:
  - gpu
  - appearance
---
# `custom_shaders = {}`

{{since('nightly')}}

Specifies a list of post-processing shaders that are applied to the rendered
terminal image after the terminal content has been drawn.  Each shader runs
as a full-screen fragment pass and can read the rendered terminal as an input
texture.  Shaders are applied in order; with more than one shader, the output
of each pass becomes the input to the next.

This option currently requires `front_end = "WebGpu"`.

A shader entry may be specified in one of two forms:

* A bare string holding a path to a native WGSL fragment shader file.  The
  shader is used as-is.
* A tagged object with `format` and `path` fields for an imported (non-native)
  shader.  Currently the only supported `format` is `"Ghostty"`.

Relative paths are resolved relative to the directory of the config file that
defined them.

## Native WGSL shaders

A native shader is a `.wgsl` file that you provide directly.  Your shader
must provide a `@fragment` entry point named `fs_postprocess` that reads from
`screen_texture` using `screen_sampler` and returns the final color.

The following uniforms are available to your shader via the `pp` uniform
buffer:

* `resolution` (`vec2<f32>`) - pixel dimensions of the rendered output
* `time` (`f32`) - seconds since the shader pipeline started
* `time_delta` (`f32`) - seconds since the previous frame
* `frame` (`u32`) - frame counter
* `current_cursor` (`vec4<f32>`) - current cursor rectangle as `(x, y, width, height)` in pixels
* `previous_cursor` (`vec4<f32>`) - previous cursor rectangle, same form as `current_cursor`
* `current_cursor_color` (`vec4<f32>`) - current cursor color (RGBA)
* `previous_cursor_color` (`vec4<f32>`) - previous cursor color (RGBA)
* `cursor_change_time` (`f32`) - seconds since the cursor last changed position or style

## Imported (Ghostty) shaders

A Ghostty shader is a GLSL file written against the
[shadertoy](https://www.shadertoy.com/) convention used by the
[Ghostty](https://ghostty.org/) terminal.  Refer to the Ghostty and shadertoy
documentation for authoring details.

The shader is cross-compiled to WGSL at build time, so existing Ghostty
and shadertoy shaders can be used without modification.

The following shadertoy-style globals are currently populated by wezterm:

* `iResolution` (`vec3`) - output resolution
* `iTime` (`float`) - seconds since the pipeline started
* `iTimeDelta` (`float`) - seconds since the previous frame
* `iFrame` (`int`) - frame counter
* `iChannel0` (`sampler2D`) - the rendered terminal image
* `iCurrentCursor` (`vec4`) - current cursor rectangle as `(x, y, width, height)`
* `iPreviousCursor` (`vec4`) - previous cursor rectangle
* `iCurrentCursorColor` (`vec4`) - current cursor color
* `iPreviousCursorColor` (`vec4`) - previous cursor color
* `iTimeCursorChange` (`float`) - seconds since the cursor last changed

## Example

Apply a native WGSL shader:

```lua
config.front_end = 'WebGpu'
config.custom_shaders = {
  '/absolute/path/to/my_effect.wgsl',
  'shaders/another_effect.wgsl', -- relative to the config file
}
```

Apply a Ghostty-format GLSL shader:

```lua
config.front_end = 'WebGpu'
config.custom_shaders = {
  { format = 'Ghostty', path = '/path/to/crt.glsl' },
}
```

Mix both kinds in the same list; they are applied in order:

```lua
config.front_end = 'WebGpu'
config.custom_shaders = {
  { format = 'Ghostty', path = 'shaders/crt.glsl' },
  'shaders/scanlines.wgsl',
}
```
