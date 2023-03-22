# CopyMode `MoveDown`

{{since('20220624-141144-bd1b7c5d')}}

Moves the CopyMode cursor position one cell down.

```lua
local wezterm = require 'wezterm'
local act = wezterm.action

return {
  key_tables = {
    copy_mode = {
      { key = 'DownArrow', mods = 'NONE', action = act.CopyMode 'MoveDown' },
    },
  },
}
```


