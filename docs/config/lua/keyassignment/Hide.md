# Hide

Hides (or minimizes, depending on the platform) the current window.

```lua
local wezterm = require 'wezterm'

return {
  keys = {
    { key = 'h', mods = 'CMD', action = wezterm.action.Hide },
  },
}
```
