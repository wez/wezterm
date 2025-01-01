---
tags:
  - gpu
---
# `webgpu_power_preference = "LowPower"`

{{since('20221119-145034-49b9839f')}}

Specifies the power preference when selecting a webgpu GPU instance.
This option is only applicable when you have configured `front_end = "WebGpu"`.

The possible values are:

* `"LowPower"` - use an integrated GPU
* `"HighPerformance"` - use a discrete GPU

You can have more fine grained control over which GPU is selected using
[webgpu_preferred_adapter](webgpu_preferred_adapter.md).
