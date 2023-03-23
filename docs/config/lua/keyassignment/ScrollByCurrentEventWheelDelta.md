# `ScrollByCurrentEventWheelDelta`

{{since('20220807-113146-c2fee766')}}

Adjusts the scroll position by the number of lines in the vertical mouse
wheel delta field of the current mouse event, provided that it is a
vertical mouse wheel event.

This example demonstrates a mouse assignment that is actually the default, so
there's not much point adding this to your config unless you also have set
[disable_default_mouse_bindings](../config/disable_default_mouse_bindings.md)
to `true`.

```lua
local act = wezterm.action

config.mouse_bindings = {
  {
    event = { Down = { streak = 1, button = { WheelUp = 1 } } },
    mods = 'NONE',
    action = act.ScrollByCurrentEventWheelDelta,
  },
  {
    event = { Down = { streak = 1, button = { WheelDown = 1 } } },
    mods = 'NONE',
    action = act.ScrollByCurrentEventWheelDelta,
  },
}
```

