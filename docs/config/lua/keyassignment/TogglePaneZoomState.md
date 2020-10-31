# TogglePaneZoomState

*Since: 20201031-154415-9614e117*

Toggles the zoom state of the current pane.  A Zoomed pane takes up
all available space in the tab, hiding all other panes while it is zoomed.
Switching its zoom state off will restore the prior split arrangement.

```lua
return {
  keys = {
    { key = "Z", mods="CTRL", action="TogglePaneZoomState" },
  }
}
```
