# ActivatePaneDirection

*Since: nightly builds only*

`ActivatePaneDirection` activate an adjacent pane in the specified direction.
In cases where there are multiple adjacent panes in the intended direction,
wezterm will choose the pane that has the largest edge intersection.

```lua
local wezterm = require 'wezterm';

return {
  keys = {
    { key = "LeftArrow", mods="CTRL|SHIFT",
      action=wezterm.action{ActivatePaneDirection="Left"}},
    { key = "RightArrow", mods="CTRL|SHIFT",
      action=wezterm.action{ActivatePaneDirection="Right"}},
    { key = "UpArrow", mods="CTRL|SHIFT",
      action=wezterm.action{ActivatePaneDirection="Up"}},
    { key = "DownArrow", mods="CTRL|SHIFT",
      action=wezterm.action{ActivatePaneDirection="Down"}},
  }
}
```
