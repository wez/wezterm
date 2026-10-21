---
tags:
  - appearance
  - background
---
# `window_background_opacity = 1.0`

{{since('20201031-154415-9614e117')}}

Specifies the alpha channel value for the window background.
If your Operating System provides compositing support, the window background
is rendered translucent (some refer to this as transparent rather than
translucent), causing the windows and desktop behind it to show through.

The value is a floating point number in the range `0.0` (completely
translucent/transparent) through to `1.0` (completely opaque), which is the
default.

```lua
config.window_background_opacity = 0.8
```

The opacity also applies to the
[window_background_image](window_background_image.md) and
[window_background_gradient](window_background_gradient.md) layers.

## Performance

Setting `window_background_opacity` to a value other than the default `1.0`
may impact render performance.

## Platform support

macOS, Windows and Wayland support compositing out of the box.
X11 may require installing or configuring a compositing window manager.
XWayland under Mutter/Wayland also works without any additional configuration.

On macOS, the window shadow is disabled automatically when the opacity is
set to less than `1.0`. You can re-enable it by adding `MACOS_FORCE_ENABLE_SHADOW`
to [window_decorations](window_decorations.md).

## Blur and backdrop effects

Opacity can be combined with operating system blur effects for a frosted
glass look rather than a crystal clear view of the desktop:

- [macos_window_background_blur](macos_window_background_blur.md)
- [wayland_window_background_blur](wayland_window_background_blur.md)
- [win32_system_backdrop](win32_system_backdrop.md)

For example, on macOS:
```lua
config.window_background_opacity = 0.3
config.macos_window_background_blur = 20
```

On Windows, you need to reduce the opacity below `1.0` for
`win32_system_backdrop` effects to work. For best results with the `"Mica"`
and `"Tabbed"` effects, set it to `0`.

With a backdrop effect you may want to raise the opacity of the non-default
background colors to keep text readable.
See [text_background_opacity](../../appearance.md#text-background-opacity).

## Toggling opacity at runtime

A common use case for opacity is to quickly peek at the desktop or another
window behind the terminal, and to toggle back to opaque for normal work.

```lua
return {
  keys = {
    {
      key = 'o',
      mods = 'CTRL',
      action = wezterm.action_callback(function(window, pane)
        local overrides = window:get_config_overrides() or {}
        if not overrides.window_background_opacity then
          -- no override yet, set the opacity
          overrides.window_background_opacity = 0.5
        else
          -- else we override already, reset opacity to default from config
          overrides.window_background_opacity = nil
        end
        window:set_config_overrides(overrides)
      end)
    },
  },
}
```
