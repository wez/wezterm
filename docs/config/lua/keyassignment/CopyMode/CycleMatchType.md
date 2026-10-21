# CopyMode `CycleMatchType`

{{since('20220624-141144-bd1b7c5d')}}

Move the CopyMode/SearchMode cycle between case-sensitive, case-insensitive,
smart-case and regular expression match types.

See [`Search` key assignment](../Search.md) for details about each match types.

```lua
local wezterm = require 'wezterm'
local act = wezterm.action

return {
  key_tables = {
    search_mode = {
      { key = 'r', mods = 'CTRL', action = act.CopyMode 'CycleMatchType' },
    },
  },
}
```
