---
tags:
  - appearance
---
# `macos_remember_window_frame = false`

{{since('nightly')}}

When set to `true`, wezterm remembers the position and size of its window
and restores them the next time a window is created, for example when
wezterm is launched again.

The frame is saved by macOS as the window is moved or resized, so the
position and size that the window had when it was last closed are used.

Only one window at a time remembers its frame: the first window that is
created uses and updates the saved frame, while any additional windows are
positioned as usual.

The saved frame is not used for a window that is spawned with an explicit
position, such as one passed to
[wezterm.mux.spawn_window](../wezterm.mux/spawn_window.md) or via
`wezterm start --position`.

While the window is in the non-native full screen mode (see
[native_macos_fullscreen_mode](native_macos_fullscreen_mode.md)), the full
screen frame is not remembered.

```lua
config.macos_remember_window_frame = true
```

This option only has an effect when running on macOS.
