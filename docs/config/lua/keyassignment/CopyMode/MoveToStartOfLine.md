# CopyMode `MoveToStartOfLine`

{{since('20220624-141144-bd1b7c5d')}}

Moves the CopyMode cursor position to the first cell in the current line.

```lua
local wezterm = require 'wezterm'
local act = wezterm.action

return {
  key_tables = {
    copy_mode = {
      {
        key = '0',
        mods = 'NONE',
        action = act.CopyMode 'MoveToStartOfLine',
      },
    },
  },
}
```

