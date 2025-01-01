# `wezterm.gui.enumerate_gpus()`

{{since('20221119-145034-49b9839f')}}

Returns the list of available Gpus supported by WebGpu.

This is useful in conjunction with [webgpu_preferred_adapter](../config/webgpu_preferred_adapter.md)

```
> wezterm.gui.enumerate_gpus()
[
    {
        "backend": "Vulkan",
        "device": 29730,
        "device_type": "DiscreteGpu",
        "driver": "radv",
        "driver_info": "Mesa 22.3.4",
        "name": "AMD Radeon Pro W6400 (RADV NAVI24)",
        "vendor": 4098,
    },
    {
        "backend": "Vulkan",
        "device": 0,
        "device_type": "Cpu",
        "driver": "llvmpipe",
        "driver_info": "Mesa 22.3.4 (LLVM 15.0.7)",
        "name": "llvmpipe (LLVM 15.0.7, 256 bits)",
        "vendor": 65541,
    },
    {
        "backend": "Gl",
        "device": 0,
        "device_type": "Other",
        "name": "AMD Radeon Pro W6400 (navi24, LLVM 15.0.7, DRM 3.49, 6.1.9-200.fc37.x86_64)",
        "vendor": 4098,
    },
]
```
