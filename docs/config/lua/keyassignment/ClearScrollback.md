# ClearScrollback

Clears the lines that have scrolled off the top of the viewport, resetting
the scrollbar thumb to the full height of the window.

*since: 20210203-095643-70a364eb*

Added a parameter that allows additionally clear the viewport:

```lua
local wezterm = require 'wezterm'
local act = wezterm.action

return {
  keys = {
    -- Clears only the scrollback and leaves the viewport intact.
    -- This is the default behavior.
    {
      key = 'K',
      mods = 'CTRL|SHIFT',
      action = act.ClearScrollback 'ScrollbackOnly',
    },
    -- Clears the scrollback and viewport leaving the prompt line the new first line.
    {
      key = 'K',
      mods = 'CTRL|SHIFT',
      action = act.ClearScrollback 'ScrollbackAndViewport',
    },
  },
}
```
