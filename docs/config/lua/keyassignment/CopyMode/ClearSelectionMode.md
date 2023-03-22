# CopyMode `ClearSelectionMode`

{{since('20220807-113146-c2fee766')}}

Clears the current CopyMode selection mode without leaving CopyMode.

```lua
local wezterm = require 'wezterm'
local act = wezterm.action

return {
  key_tables = {
    copy_mode = {
      {
        key = 'y',
        mods = 'NONE',
        action = act.Multiple {
          act.CopyTo 'PrimarySelection',
          act.ClearSelection,
          -- clear the selection mode, but remain in copy mode
          act.CopyMode { 'ClearSelectionMode' },
        },
      },
    },
  },
}
```

See also: [SetSelectionMode](SetSelectionMode.md).
