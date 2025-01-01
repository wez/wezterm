---
tags:
  - tab_bar
---
# `show_tabs_in_tab_bar = true`

{{since('20221119-145034-49b9839f')}}

When set to `true` (the default), the tab bar will display the tabs associated
with the current window.

When set to `false`, the tabs will not be drawn into the tab bar.

This example turns off the tabs and new-tab button, leaving just the left and
right status areas:

```lua
wezterm.on('update-right-status', function(window, pane)
  window:set_left_status 'left'
  window:set_right_status 'right'
end)

config.use_fancy_tab_bar = false
config.show_tabs_in_tab_bar = false
config.show_new_tab_button_in_tab_bar = false
```
