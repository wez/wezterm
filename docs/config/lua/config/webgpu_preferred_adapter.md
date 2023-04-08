---
tags:
  - gpu
---
# `webgpu_preferred_adapter`

{{since('20221119-145034-49b9839f')}}

Specifies which WebGpu adapter should be used.

This option is only applicable when you have configured `front_end = "WebGpu"`.

You can use the [wezterm.gui.enumerate_gpus()](../wezterm.gui/enumerate_gpus.md) function
to return a list of GPUs.

If you open the [Debug Overlay](../keyassignment/ShowDebugOverlay.md) (default:
<kbd>CTRL</kbd> + <kbd>SHIFT</kbd> + <kbd>L</kbd>) you can interactively review
the list:

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

Based on that list, I might choose to explicitly target the discrete Gpu like
this (but note that this would be the default selection anyway):

```lua
config.webgpu_preferred_adapter = {
  backend = 'Vulkan',
  device = 29730,
  device_type = 'DiscreteGpu',
  driver = 'radv',
  driver_info = 'Mesa 22.3.4',
  name = 'AMD Radeon Pro W6400 (RADV NAVI24)',
  vendor = 4098,
}
config.front_end = 'WebGpu'
```

alternatively, I might use:

```lua
local wezterm = require 'wezterm'
local config = {}
local gpus = wezterm.gui.enumerate_gpus()

config.webgpu_preferred_adapter = gpus[1]
config.front_end = 'WebGpu'
return config
```

If you have a more complex situation you can get a bit more elaborate; this
example will only enable WebGpu if there is an integrated GPU available with
Vulkan drivers:

```lua
local wezterm = require 'wezterm'
local config = {}

for _, gpu in ipairs(wezterm.gui.enumerate_gpus()) do
  if gpu.backend == 'Vulkan' and gpu.device_type == 'IntegratedGpu' then
    config.webgpu_preferred_adapter = gpu
    config.front_end = 'WebGpu'
    break
  end
end

return config
```

See also [webgpu_power_preference](webgpu_power_preference.md),
[webgpu_force_fallback_adapter](webgpu_force_fallback_adapter.md).
