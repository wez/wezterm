# `native_macos_fullscreen_mode = false`

Specifies whether the [ToggleFullScreen](../../lua/keyassignment/ToggleFullScreen.md)
key assignment uses the native macOS full-screen application support or not.

The default is `false` which will simply (and very quickly!) toggle between a
full screen window with no decorations and a regularly size window.

When `true`, transitioning to full screen will slowly animate the window moving
to a full screen space on the monitor.

This option only has an effect when running on macOS.
