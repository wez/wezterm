# `tab_bar_style`

*Since: 20210502-154244-3f7122cb*

`active_tab_left`, `active_tab_right`, `inactive_tab_left`,
`inactive_tab_right`, `inactive_tab_hover_left`, `inactive_tab_hover_right`
have been removed and replaced by the more flexible
[format-tab-title](../window-events/format-tab-title.md) event.

*Since: 20210314-114017-04b7cedd*

This config option allows styling the elements that appear in the tab bar.
This configuration supplements the [tab bar color](../../appearance.html#tab-bar-appearance--colors)
options.

Styling in this context refers to how the edges of the tabs and the new tab button are rendered.
The default is simply a space character but you can use any sequence of formatted text produced
by the [wezterm.format](../wezterm/format.md) function.

The defaults for each of these styles is simply a space.  For each element, the foreground
and background colors are set as per the tab bar colors you've configured.

The available elements are:

* `active_tab_left`, `active_tab_right` - the left and right sides of the active tab
* `inactive_tab_left`, `inactive_tab_right` - the left and right sides of inactive tabs
* `inactive_tab_hover_left`, `inactive_tab_hover_right` - the left and right sides of inactive tabs in the hover state
* `new_tab_left`, `new_tab_right` - the left and right sides of the new tab `+` button
* `new_tab_hover_left`, `new_tab_hover_right` - the left and right sides of the new tab `+` button in the hover state.

This example changes the tab edges to the PowerLine arrow symbols:

<img width="100%" height="100%" src="../../../screenshots/wezterm-tab-edge-styled.png"
  alt="Demonstrating setting the styling of the left and right tab edges">

```lua
local wezterm = require 'wezterm';

-- The filled in variant of the < symbol
local SOLID_LEFT_ARROW = utf8.char(0xe0b2)

-- The filled in variant of the > symbol
local SOLID_RIGHT_ARROW = utf8.char(0xe0b0)

return {
  tab_bar_style = {
    active_tab_left = wezterm.format({
      {Background={Color="#0b0022"}},
      {Foreground={Color="#2b2042"}},
      {Text=SOLID_LEFT_ARROW},
    }),
    active_tab_right = wezterm.format({
      {Background={Color="#0b0022"}},
      {Foreground={Color="#2b2042"}},
      {Text=SOLID_RIGHT_ARROW},
    }),
    inactive_tab_left = wezterm.format({
      {Background={Color="#0b0022"}},
      {Foreground={Color="#1b1032"}},
      {Text=SOLID_LEFT_ARROW},
    }),
    inactive_tab_right = wezterm.format({
      {Background={Color="#0b0022"}},
      {Foreground={Color="#1b1032"}},
      {Text=SOLID_RIGHT_ARROW},
    }),
  }
}
```

