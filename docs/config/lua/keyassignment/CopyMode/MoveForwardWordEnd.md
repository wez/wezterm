# CopyMode 'MoveForwardWord'

--8<-- "nightly-only.md"

Moves the CopyMode cursor position forward to the end of word.

```lua
local wezterm = require 'wezterm'
local act = wezterm.action

return {
  key_tables = {
    copy_mode = {
      {
        key = 'e',
        mods = 'NONE',
        action = act.CopyMode 'MoveForwardWordEnd',
      },
    },
  },
}
```

