# ShowLauncher

Activate the [Launcher Menu](../../launch.md#the-launcher-menu)
in the current tab.

```lua
local wezterm = require 'wezterm'

return {
  keys = {
    { key = 'l', mods = 'ALT', action = wezterm.action.ShowLauncher },
  },
}
```


