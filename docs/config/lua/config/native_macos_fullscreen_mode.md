---
tags:
  - appearance
---
# `native_macos_fullscreen_mode = false`

Specifies whether the [ToggleFullScreen](../../lua/keyassignment/ToggleFullScreen.md)
key assignment uses the native macOS full-screen application support or not.

The default is `false` which will simply (and very quickly!) toggle between a
window that covers the full screen, with no decorations and a regularly sized
window.

When `true`, transitioning to full screen will use the macOS native full screen
mode, which in more recent versions of macOS, will allocate a separate Space
for the wezterm application and then slowly animate the wezterm window moving
into to that full screen Space on the monitor.

This option only has an effect when running on macOS.
