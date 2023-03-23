# `StartWindowDrag`

{{since('20210314-114017-04b7cedd')}}

Places the window in the drag-to-move state, which means that the window will
move to follow your mouse pointer until the mouse button is released.

By default this is bound to SUPER + left mouse drag, as well as CTRL-SHIFT + left mouse drag.

```lua
config.mouse_bindings = {
  {
    event = { Drag = { streak = 1, button = 'Left' } },
    mods = 'SUPER',
    action = wezterm.action.StartWindowDrag,
  },
  {
    event = { Drag = { streak = 1, button = 'Left' } },
    mods = 'CTRL|SHIFT',
    action = wezterm.action.StartWindowDrag,
  },
}
```
