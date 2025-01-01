# `ActivateTabRelative`

Activate a tab relative to the current tab.  The argument value specifies an
offset. eg: `-1` activates the tab to the left of the current tab, while `1`
activates the tab to the right.

```lua
local wezterm = require 'wezterm'
local act = wezterm.action
local config = {}

config.keys = {
  { key = '{', mods = 'ALT', action = act.ActivateTabRelative(-1) },
  { key = '}', mods = 'ALT', action = act.ActivateTabRelative(1) },
}

return config
```

See also [ActivateTabRelativeNoWrap](ActivateTabRelativeNoWrap.md)


