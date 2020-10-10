# ActivateTabRelative

Activate a tab relative to the current tab.  The argument value specifies an
offset. eg: `-1` activates the tab to the left of the current tab, while `1`
activates the tab to the right.

```lua
local wezterm = require 'wezterm';
return {
  keys = {
    {key="{", mods="SHIFT|ALT", action=wezterm.action{ActivateTabRelative=-1}},
    {key="}", mods="SHIFT|ALT", action=wezterm.action{ActivateTabRelative=1}},
  }
}
```


