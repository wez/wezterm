# ActivateWindowRelative(delta)

*since: nightly builds only*

Activates a GUI window relative to the current window.

`ActivateWindowRelative(1)` activates the next window, while
`ActivateWindowRelative(-1)` activates the previous window.

This action will wrap around and activate the appropriate window
at the start/end.

Here's an example of setting up (not very useful) hotkeys to cycle between
windows:

```lua
local wezterm = require 'wezterm'
local act = wezterm.action

return {
  keys = {
    { key = 'r', mods = 'ALT', action = act.ActivateWindowRelative(1) },
    { key = 'e', mods = 'ALT', action = act.ActivateWindowRelative(-1) },
  },
}
```

See also [ActivateWindowRelativeNoWrap](ActivateWindowRelativeNoWrap.md),
[ActivateWindow](ActivateWindow.md).
