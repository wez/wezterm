---
title: wezterm.action_callback
tags:
 - keys
 - event
---

# `wezterm.action_callback(callback)`

{{since('20211204-082213-a66c61ee9')}}

This function is a helper to register a custom event and return an action triggering it.

It is helpful to write custom key bindings directly, without having to declare
the event and use it in a different place.

The implementation is essentially the same as:
```lua
function wezterm.action_callback(callback)
  local event_id = '...' -- the function generates a unique event id
  wezterm.on(event_id, callback)
  return wezterm.action.EmitEvent(event_id)
end
```

See [wezterm.on](./on.md) and [wezterm.action](./action.md) for more info on what you can do with these.


## Usage

```lua
local wezterm = require 'wezterm'

return {
  keys = {
    {
      mods = 'CTRL|SHIFT',
      key = 'i',
      action = wezterm.action_callback(function(win, pane)
        wezterm.log_info 'Hello from callback!'
        wezterm.log_info(
          'WindowID:',
          win:window_id(),
          'PaneID:',
          pane:pane_id()
        )
      end),
    },
  },
}
```
