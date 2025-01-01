# `window:current_event()`

{{since('20220807-113146-c2fee766')}}

Returns the current event.
For now only implemented for mouse events.

This example prints the delta scroll value
when you scroll up with your mouse wheel while holding `CTRL`:

```lua
local wezterm = require 'wezterm'

return {
  mouse_bindings = {
    {
      event = { Down = { streak = 1, button = { WheelUp = 1 } } },
      mods = 'CTRL',
      action = wezterm.action_callback(function(window, pane)
        -- note that you want `WheelDown` for a `WheelDown` event
        local delta = window:current_event().Down.button.WheelUp
        wezterm.log_info('delta is: ' .. delta)
      end),
    },
  },
}
```
