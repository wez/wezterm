# CopyMode `MoveToSelectionOtherEnd`

{{since('20220624-141144-bd1b7c5d')}}

Moves the CopyMode cursor position to the other end of the selection; if the
cursor is at the top left corner and the starting point is the bottom right
corner, then the cursor and starting point are swapped, with the cursor now
positioned at the bottom right corner.

```lua
local wezterm = require 'wezterm'
local act = wezterm.action

return {
  key_tables = {
    copy_mode = {
      {
        key = 'o',
        mods = 'NONE',
        action = act.CopyMode 'MoveToSelectionOtherEnd',
      },
    },
  },
}
```

