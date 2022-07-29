# `window:current_event()`

*Since: nightly builds only*

Returns the current event.
For now only implemented for mouse events.

This example prints the delta scroll value
when you scroll up with your mouse wheel while holding `CTRL`:

```lua
local wezterm = require 'wezterm'

return {
  mouse_bindings = {
    {
      event = 'WheelUp',
      mods = 'CTRL',
      action = wezterm.action_callback(function(window, pane)
        local delta = window:current_event()['Down']['button']['WheelUp']
        wezterm.log_info('delta is: ' .. delta)
      end),
    },
  },
}
```
