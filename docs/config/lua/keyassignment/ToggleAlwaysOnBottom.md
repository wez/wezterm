# `ToggleAlwaysOnBottom`

{{since('20240127-113634-bbcac864')}}

Toggles the window to remain behind all other windows.

```lua
config.keys = {
  {
    key = ']',
    mods = 'CMD|SHIFT',
    action = wezterm.action.ToggleAlwaysOnBottom,
  },
}
```

!!! note
    This functionality is currently only implemented on macOS. 
    The assigned values for window level will have no effect on other operating systems.
