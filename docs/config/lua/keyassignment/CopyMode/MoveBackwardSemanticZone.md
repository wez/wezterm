# CopyMode 'MoveBackwardSemanticZone'

*Since: nightly builds only*

Moves the CopyMode cursor position one semantic zone to the left.

See [Shell Integration](../../../../shell-integration.md) for more information
about semantic zones.

```lua
local wezterm = require 'wezterm'
local act = wezterm.action

return {
  key_tables = {
    copy_mode = {
      {
        key = 'z',
        mods = 'NONE',
        action = act.CopyMode 'MoveBackwardSemanticZone',
      },
    },
  },
}
```

