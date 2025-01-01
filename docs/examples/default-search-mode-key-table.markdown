```lua
local wezterm = require 'wezterm'
local act = wezterm.action

return {
  key_tables = {
    search_mode = {
      { key = 'Enter', mods = 'NONE', action = act.CopyMode 'PriorMatch' },
      { key = 'Escape', mods = 'NONE', action = act.CopyMode 'Close' },
      { key = 'n', mods = 'CTRL', action = act.CopyMode 'NextMatch' },
      { key = 'p', mods = 'CTRL', action = act.CopyMode 'PriorMatch' },
      { key = 'r', mods = 'CTRL', action = act.CopyMode 'CycleMatchType' },
      { key = 'u', mods = 'CTRL', action = act.CopyMode 'ClearPattern' },
      {
        key = 'PageUp',
        mods = 'NONE',
        action = act.CopyMode 'PriorMatchPage',
      },
      {
        key = 'PageDown',
        mods = 'NONE',
        action = act.CopyMode 'NextMatchPage',
      },
      { key = 'UpArrow', mods = 'NONE', action = act.CopyMode 'PriorMatch' },
      { key = 'DownArrow', mods = 'NONE', action = act.CopyMode 'NextMatch' },
    },
  },
}
```
