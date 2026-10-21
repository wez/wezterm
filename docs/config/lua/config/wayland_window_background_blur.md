---
tags:
    - appearance
---
# wayland_window_background_blur = false

When combined with [window_background_opacity](window_background_opacity.md),
enables background blur using the Wayland background effect protocol.

This can be used to produce a translucent window effect rather than
a crystal clear transparent window effect.

This effect can be achieved by adding the following to the configuration:
```lua
config.window_background_opacity = 0.4
config.wayland_window_background_blur = true
```

!!! note
    Wayland compositors may need to have blur enabled explicitly.
    e.g. on KDE Plasma, enable the _Blur_ plugin in _Window Effect_ settings.

[Screenshot](../../../screenshots/wezterm-ext-background-effects-v1.png)

See also [win32_system_backdrop](./win32_system_backdrop.md) for a similar
effect on Windows

See also [macos_window_background_blur](./macos_window_background_blur.md) for
a similar effect on macOS
