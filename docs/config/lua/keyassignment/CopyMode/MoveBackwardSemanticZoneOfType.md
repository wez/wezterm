# CopyMode `{ MoveBackwardSemanticZone = ZONE }`

{{since('20220903-194523-3bb1ed61')}}

Moves the CopyMode cursor position to the first semantic zone of the specified
type that precedes the current zone.

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
        key = 'z',
        mods = 'ALT',
        action = act.CopyMode { MoveBackwardZoneOfType = 'Output' },
      },
    },
  },
}
```


