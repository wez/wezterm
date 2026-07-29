---
tags:
  - appearance
---

# `macos_fullscreen_extend_behind_notch = false`

{{since('nightly')}}

When `true` and in full screen mode, the window will extend behind 
the notch on macOS.

The default value for `macos_fullscreen_extend_behind_notch` is `false`.

Must be used with `native_macos_fullscreen_mode` set to `false`.

Toggling full screen with the native macOS full screen button or
a window manager command won't have any effect and you must use the 
"Toggle full screen mode" button in `View > Toggle full screen mode`
or configure your own key, see [ToggleFullScreen](../keyassignment/ToggleFullScreen.md).

Example config:

```lua
config.native_macos_fullscreen_mode = false
config.macos_fullscreen_extend_behind_notch = true
```

This option only has an effect when running on macOS.

See [native_macos_fullscreen_mode](native_macos_fullscreen_mode.md) on how to 
disable native full screen mode on macOS.

