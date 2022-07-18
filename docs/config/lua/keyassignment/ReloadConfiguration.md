# ReloadConfiguration

Explicitly reload the configuration.

```lua
local wezterm = require 'wezterm'

return {
  keys = {
    {
      key = 'r',
      mods = 'CMD|SHIFT',
      action = wezterm.action.ReloadConfiguration,
    },
  },
}
```


