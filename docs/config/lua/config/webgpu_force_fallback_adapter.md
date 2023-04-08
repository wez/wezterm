---
tags:
  - gpu
---
# `webgpu_force_fallback_adapter = false`

{{since('20221119-145034-49b9839f')}}

If set to `true`, forces the use of a fallback software (CPU based) rendering
backend.  The performance will not be as good as using a GPU.

This option is only applicable when you have configured `front_end = "WebGpu"`.

You can have more fine grained control over which GPU is selected using
[webgpu_preferred_adapter](webgpu_preferred_adapter.md).
