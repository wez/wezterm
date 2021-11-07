# ActivatePaneDirection

*Since: 20201031-154415-9614e117*

`ActivatePaneDirection` activate an adjacent pane in the specified direction.
In cases where there are multiple adjacent panes in the intended direction,
wezterm will choose the pane that has the largest edge intersection.

If the active pane is [zoomed](TogglePaneZoomState.md), behavior is determined
by the [`unzoom_on_switch_pane`](../config/unzoom_on_switch_pane.md) flag. 

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
