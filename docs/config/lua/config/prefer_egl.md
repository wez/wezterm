---
tags:
  - tuning
---
# `prefer_egl = true`

Depending on the OS and windowing environment, there are a number of different
ways to access the GPU.

This option controls whether wezterm should attempt to use
[EGL](https://en.wikipedia.org/wiki/EGL_%28API%29) to configure the GPU.

| Environment | prefer_egl=true | prefer_egl=false |
|-------------|-----------------|------------------|
| Wayland     | use wayland-egl | No effect: EGL is the only way to use the GPU |
| X11         | Use libEGL.so   | No effect: EGL is the only way to use the GPU |
| macOS       | Use [MetalANGLE](https://github.com/kakashidinho/metalangle) libEGL.dylib if installed alongside the `wezterm-gui` executable.  Some versions of wezterm shipped with this configuration which translated OpenGL calls to [Metal](https://en.wikipedia.org/wiki/Metal_%28API%29) | Use Core OpenGL API (CGL).  Since BigSur, CGL uses Metal APIs. |
| Windows     | Use [ANGLE](https://chromium.googlesource.com/angle/angle) to translate OpenGL calls to Direct3D, which makes wezterm more robust if you upgrade your graphics card drivers. | Use the OpenGL implementation provided by your graphics card vendor |

The default is `true`.
