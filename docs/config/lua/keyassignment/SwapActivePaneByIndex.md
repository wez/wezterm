# `SwapActivePaneByIndex`

{{since('20220319-142410-0fcdea07')}}

`SwapActivePaneByIndex` swaps the active pane with the pane with the specified
index within the current tab.  Invalid indices are ignored.

This example causes CTRL-ALT-a, CTRL-ALT-b, CTRL-ALT-c to swap the current pane
with the 0th, 1st and 2nd panes, respectively:

```lua
local wezterm = require 'wezterm'
local act = wezterm.action
local config = {}

config.keys = {
  {
    key = 'a',
    mods = 'CTRL|ALT',
    action = { SwapActivePaneByIndex = { pane_index = 0, keep_focus = true } },
  },
  {
    key = 'b',
    mods = 'CTRL|ALT',
    action = { SwapActivePaneByIndex = { pane_index = 1, keep_focus = true } },
  },
  {
    key = 'c',
    mods = 'CTRL|ALT',
    action = { SwapActivePaneByIndex = { pane_index = 2, keep_focus = true } },
  },
}

return config
```
