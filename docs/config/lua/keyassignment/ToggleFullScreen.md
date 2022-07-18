# ToggleFullScreen

Toggles full screen mode for the current window.

```lua
local wezterm = require 'wezterm'

return {
  keys = {
    {
      key = 'n',
      mods = 'SHIFT|CTRL',
      action = wezterm.action.ToggleFullScreen,
    },
  },
}
```


