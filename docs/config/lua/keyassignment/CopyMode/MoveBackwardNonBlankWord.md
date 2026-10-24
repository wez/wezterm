# CopyMode `MoveBackwardNonBlankWord`

{{since('nightly')}}

Moves the CopyMode cursor position one non-blank word to the left.

```lua
local wezterm = require 'wezterm'
local act = wezterm.action

return {
  key_tables = {
    copy_mode = {
      {
        key = 'B',
        mods = 'NONE',
        action = act.CopyMode 'MoveBackwardNonBlankWord',
      },
    },
  },
}
```
