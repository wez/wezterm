---
tags:
  - tab_bar
---
# `show_close_tab_button_in_tab = true`

{{since('nightly')}}

When set to `true` (the default), the tab bar will display the close-tab
button in each tab, which can be left-clicked to close the tab.

When set to `false`, the close-tab button will not be drawn into each tab.

This example turns off the tabs, new-tab button, and close-tab buttons, leaving
just the left and right status areas:

```lua
wezterm.on('update-right-status', function(window, pane)
  window:set_left_status 'left'
  window:set_right_status 'right'
end)

config.use_fancy_tab_bar = false
config.show_tabs_in_tab_bar = false
config.show_new_tab_button_in_tab_bar = false
config.show_close_tab_button_in_tab = false
```
