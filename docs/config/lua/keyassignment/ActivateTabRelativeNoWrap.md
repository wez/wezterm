# `ActivateTabRelativeNoWrap`

{{since('20220101-133340-7edc5b5a')}}

Activate a tab relative to the current tab.  The argument value specifies an
offset. eg: `-1` activates the tab to the left of the current tab, while `1`
activates the tab to the right.

This is almost identical to [ActivateTabRelative](ActivateTabRelative.md)
but this one will not wrap around; for example, if the first tab is active
`ActivateTabRelativeNoWrap=-1` will not move to the last tab and vice versa.


```lua
local wezterm = require 'wezterm'
local act = wezterm.action
local config = {}

config.keys = {
  { key = '{', mods = 'ALT', action = act.ActivateTabRelativeNoWrap(-1) },
  { key = '}', mods = 'ALT', action = act.ActivateTabRelativeNoWrap(1) },
}
return config
```


