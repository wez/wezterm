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

*Since: 20220101-133340-7edc5b5a*

You may now use `"Next"` and `"Prev"` as directions.  These cycle
through the panes according to their position in the pane tree.

`"Next"` moves to the next highest pane index, wrapping around to 0
if the active pane is already the highest pane index.

`"Prev"` moves to the next lowest pane index, wrapping around to
the highest of the active pane is already the lowest pane index.
