# CopyMode 'MoveForewardSemanticZone'

*Since: nightly builds only*

Moves the CopyMode cursor position one semantic zone to the right.

See [Shell Integration](../../../../shell-integration.md) for more information
about semantic zones.

```lua
local wezterm = require 'wezterm'
local act = wezterm.action

return {
  key_tables = {
    copy_mode = {
      {
        key = 'Z',
        mods = 'NONE',
        action = act.CopyMode 'MoveForewardSemanticZone',
      },
    },
  },
}
```


