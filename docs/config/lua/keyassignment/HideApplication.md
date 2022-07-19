# HideApplication

On macOS, hide the WezTerm application.

```lua
local wezterm = require 'wezterm'

return {
  keys = {
    { key = 'h', mods = 'CMD', action = wezterm.action.HideApplication },
  },
}
```
