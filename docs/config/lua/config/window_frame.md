# window_frame

*Since: 20210814-124438-54e29167*

This setting is applicable primarily on Wayland systems when client side
decorations are in use.

It allows you to customize the colors of the window frame.

Some of these colors are used by the fancy tab bar.

```lua
return {
  window_frame = {
    inactive_titlebar_bg = '#353535',
    active_titlebar_bg = '#2b2042',
    inactive_titlebar_fg = '#cccccc',
    active_titlebar_fg = '#ffffff',
    inactive_titlebar_border_bottom = '#2b2042',
    active_titlebar_border_bottom = '#2b2042',
    button_fg = '#cccccc',
    button_bg = '#2b2042',
    button_hover_fg = '#ffffff',
    button_hover_bg = '#3b3052',
  },
}
```

*Since: nightly builds only*

You may explicitly add a border around the window area:

```lua
return {
  window_frame = {
    border_left_width = '0.5cell',
    border_right_width = '0.5cell',
    border_bottom_height = '0.25cell',
    border_top_height = '0.25cell',
    border_left_color = 'purple',
    border_right_color = 'purple',
    border_bottom_color = 'purple',
    border_top_color = 'purple',
  },
}
```
