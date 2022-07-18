# MoveTabRelative

Move the current tab relative to its peers.  The argument specifies an
offset. eg: `-1` moves the tab to the left of the current tab, while `1` moves
the tab to the right.

```lua
local wezterm = require 'wezterm'
local act = wezterm.action

return {
  keys = {
    { key = '{', mods = 'SHIFT|ALT', action = act.MoveTabRelative(-1) },
    { key = '}', mods = 'SHIFT|ALT', action = act.MoveTabRelative(1) },
  },
}
```


