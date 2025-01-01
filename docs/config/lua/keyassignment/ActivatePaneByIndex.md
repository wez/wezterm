# `ActivatePaneByIndex`

{{since('20220319-142410-0fcdea07')}}

`ActivatePaneByIndex` activates the pane with the specified index within
the current tab.  Invalid indices are ignored.

This example causes ALT-a, ALT-b, ALT-c to switch to the 0th, 1st and 2nd
panes, respectively:

```lua
local wezterm = require 'wezterm'
local act = wezterm.action
local config = {}

config.keys = {
  { key = 'a', mods = 'ALT', action = act.ActivatePaneByIndex(0) },
  { key = 'b', mods = 'ALT', action = act.ActivatePaneByIndex(1) },
  { key = 'c', mods = 'ALT', action = act.ActivatePaneByIndex(2) },
}

return config
```
