# ScrollByPage

Adjusts the scroll position by the number of pages specified by the argument.
Negative values scroll upwards, while positive values scroll downwards.

```lua
local wezterm = require 'wezterm'
local act = wezterm.action

return {
  keys = {
    { key = 'PageUp', mods = 'SHIFT', action = act.ScrollByPage(-1) },
    { key = 'PageDown', mods = 'SHIFT', action = act.ScrollByPage(1) },
  },
}
```

*Since: 20220319-142410-0fcdea07*

You may now use floating point values to scroll by partial pages.  This example shows
how to make the `PageUp`/`PageDown` scroll by half a page at a time:

```lua
local wezterm = require 'wezterm'

return {
  keys = {
    { key = 'PageUp', mods = 'SHIFT', action = act.ScrollByPage(-0.5) },
    { key = 'PageDown', mods = 'SHIFT', action = act.ScrollByPage(0.5) },
  },
}
```
