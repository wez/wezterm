# `ToggleAlwaysOnTop`

Toggles the window between floating and non-floating states to stay on top of other windows.

```lua
local wezterm = require 'wezterm'
local config = {}

config.keys = {
  {
    key = ']',
    mods = 'CMD|SHIFT',
    action = wezterm.action.ToggleAlwaysOnTop,
  },
}
```


