## SpawnCommandInNewTab

Spawn a new tab into the current window.
The argument is a `SpawnCommand` struct that is discussed in more
detail in the [SpawnCommand](../SpawnCommand.md) docs.

```lua
local wezterm = require 'wezterm'

return {
  keys = {
    -- CMD-y starts `top` in a new window
    {key="y", mods="CMD", action=wezterm.action.SpawnCommandInNewTab{
      args={"top"}
    }},
  }
}
```


