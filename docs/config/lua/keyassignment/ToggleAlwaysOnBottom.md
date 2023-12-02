# `ToggleAlwaysOnBottom`

Toggles the window to remain behind all other windows.

```lua
local wezterm = require 'wezterm'

config.keys = {
  {
    key = ']',
    mods = 'CMD|SHIFT',
    action = wezterm.action.ToggleAlwaysOnBottom,
  },
}
```


