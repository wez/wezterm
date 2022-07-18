# Nop

Causes the key press to have no effect; it behaves as though those
keys were not pressed.

If instead of this you want the key presses to pass through to
the terminal, look at [DisableDefaultAssignment](DisableDefaultAssignment.md).

```lua
local wezterm = require 'wezterm'

return {
  keys = {
    -- Turn off any side effects from pressing CMD-m
    { key = 'm', mods = 'CMD', action = wezterm.action.Nop },
  },
}
```

