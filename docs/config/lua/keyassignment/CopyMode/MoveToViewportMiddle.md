# CopyMode `MoveToViewportMiddle`

{{since('20220624-141144-bd1b7c5d')}}

Moves the CopyMode cursor position to the middle of the viewport.


```lua
local wezterm = require 'wezterm'
local act = wezterm.action

return {
  key_tables = {
    copy_mode = {
      {
        key = 'M',
        mods = 'NONE',
        action = act.CopyMode 'MoveToViewportMiddle',
      },
    },
  },
}
```

