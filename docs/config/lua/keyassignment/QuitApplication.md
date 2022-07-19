# QuitApplication

Terminate the WezTerm application, killing all tabs.

```lua
local wezterm = require 'wezterm'

return {
  keys = {
    { key = 'q', mods = 'CMD', action = wezterm.action.QuitApplication },
  },
}
```


