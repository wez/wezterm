# window_frame

*Since: nightly builds only*

This setting is applicable only on Wayland systems when client side decorations are in use.

It allows you to customize the colors of the window frame.

```lua
return {
  window_frame = {
    inactive_titlebar_bg = "#353535",
    active_titlebar_bg = "#2b2042",
    inactive_titlebar_fg = "#cccccc",
    active_titlebar_fg = "#ffffff",
    inactive_titlebar_border_bottom = "#2b2042",
    active_titlebar_border_bottom = "#2b2042",
    button_fg = "#cccccc",
    button_bg = "#2b2042",
    button_hover_fg = "#ffffff",
    button_hover_bg = "#3b3052",
  }
}
```
