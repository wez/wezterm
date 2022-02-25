# ActivatePaneByIndex

*Since: nightly builds only*

`ActivatePaneByIndex` activates the pane with the specified index within
the current tab.  Invalid indices are ignored.

This example causes ALT-a, ALT-b, ALT-c to switch to the 0th, 1st and 2nd
panes, respectively:

```lua
local wezterm = require 'wezterm';

return {
  keys = {
    {key="a", mods="ALT", action=wezterm.action{ActivatePaneByIndex=0}},
    {key="b", mods="ALT", action=wezterm.action{ActivatePaneByIndex=1}},
    {key="c", mods="ALT", action=wezterm.action{ActivatePaneByIndex=2}},
  }
}
```
