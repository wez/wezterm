# ScrollByCurrentEventWheelDelta

*Since: nightly builds only*

Adjusts the scroll position by the number of lines in the vertical mouse
wheel delta field of the current mouse event, provided that it is a
vertical mouse wheel event.

This example demonstrates a mouse assignment that is actually the default, so
there's not much point adding this to your config unless you also have set
[disable_default_mouse_bindings](../config/disable_default_mouse_bindings.md)
to `true`.

```lua
local wezterm = require 'wezterm'
local act = wezterm.action

return {
  mouse_bindings = {
    {
      event = 'WheelUp',
      mods = 'NONE',
      action = act.ScrollByCurrentEventWheelDelta,
    },
    {
      event = 'WheelDown',
      mods = 'NONE',
      action = act.ScrollByCurrentEventWheelDelta,
    },
  },
}
```

