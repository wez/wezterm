# TogglePaneZoomState

*Since: nightly builds only*

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
