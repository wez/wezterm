# CopyMode { MoveForwardSemanticZone = ZONE }

*Since: nightly builds only*

Moves the CopyMode cursor position to the next semantic zone of the specified
type that follows the current zone.

See [Shell Integration](../../../../shell-integration.md) for more information
about semantic zones.

Possible values for ZONE are:

* `"Output"`
* `"Input"`
* `"Prompt"`

```lua
local wezterm = require 'wezterm'
local act = wezterm.action

return {
  key_tables = {
    copy_mode = {
      {
        key = 'Z',
        mods = 'ALT',
        action = act.CopyMode { MoveForwardZoneOfType = 'Output' },
      },
    },
  },
}
```



