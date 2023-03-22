# `ActivateWindowRelative(delta)`

{{since('20230320-124340-559cb7b0')}}

Activates a GUI window relative to the current window.

`ActivateWindowRelativeNoWrap(1)` activates the next window, while
`ActivateWindowRelativeNoWrap(-1)` activates the previous window.

This action will NOT wrap around; if the current window is the first/last, then this action will not change the current window.

Here's an example of setting up (not very useful) hotkeys to cycle between
windows:

```lua
local wezterm = require 'wezterm'
local act = wezterm.action
local config = {}

config.keys = {
  {
    key = 'r',
    mods = 'ALT',
    action = act.ActivateWindowRelativeNoWrap(1),
  },
  {
    key = 'e',
    mods = 'ALT',
    action = act.ActivateWindowRelativeNoWrap(-1),
  },
}
return config
```

See also [ActivateWindowRelative](ActivateWindowRelative.md),
[ActivateWindow](ActivateWindow.md).
