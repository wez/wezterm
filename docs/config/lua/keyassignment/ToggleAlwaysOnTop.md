# `ToggleAlwaysOnTop`

{{since('nightly')}}

Toggles the window between floating and non-floating states to stay on top of other windows.

```lua
config.keys = {
  {
    key = ']',
    mods = 'CMD|SHIFT',
    action = wezterm.action.ToggleAlwaysOnTop,
  },
}
```

!!! note 
    This functionality is currently only implemented on macOS. 
    The assigned values for window level will have no effect on other operating systems.
