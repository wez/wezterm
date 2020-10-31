# SplitVertical

*Since: 20201031-154415-9614e117*

Splits the current pane in half vertically such that the current pane becomes
the top half and the new bottom half spawns a new command.

`SplitVertical` requires a [SpawnCommand](../SpawnCommand.md) parameter to
specify what should be spawned into the new split.

```lua
local wezterm = require 'wezterm';

return {
  keys = {
    {key="\"", mods="CTRL|SHIFT|ALT", action=wezterm.action{SplitVertical={
      args={"top"}
    }}},
  }
}
```

