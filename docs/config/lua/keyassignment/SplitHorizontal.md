# SplitHorizontal

*Since: 20201031-154415-9614e117*

Splits the current pane in half horizontally such that the current pane becomes
the left half and the new right half spawns a new command.

`SplitHorizontal` requires a [SpawnCommand](../SpawnCommand.md) parameter to
specify what should be spawned into the new split.

```lua
local wezterm = require 'wezterm';

return {
  keys = {
    {key="%", mods="CLTR|SHIFT|ALT", action=wezterm.action{SplitHorizontal={
      args={"top"}
    }}},
  }
}
```

