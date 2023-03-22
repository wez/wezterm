# `QuickSelect`

{{since('20210502-130208-bff6815d')}}

Activates [Quick Select Mode](../../../quickselect.md).

```lua
local wezterm = require 'wezterm'

config.keys = {
  { key = ' ', mods = 'SHIFT|CTRL', action = wezterm.action.QuickSelect },
}
```

See also [QuickSelectArgs](QuickSelectArgs.md)
