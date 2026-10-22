# `TogglePaneInputSynchronization`

{{since('nightly')}}

Toggles synchronized input for panes in the current tab.

When synchronization is enabled, keyboard input sent to the active pane is
also mirrored to the other panes in that same tab.

```lua
local wezterm = require 'wezterm'
local act = wezterm.action

return {
  keys = {
    {
      key = 'I',
      mods = 'CTRL|SHIFT',
      action = act.TogglePaneInputSynchronization,
    },
  },
}
```

This setting is scoped per tab.
