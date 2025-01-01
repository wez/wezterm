# `mux-startup`

{{since('20220624-141144-bd1b7c5d')}}

The `mux-startup` event is emitted once when the mux server is starting up.
It is triggered before any default program is started.

If the `mux-startup` event causes any panes to be created then those will
take precedence over the default program configuration and no additional
default program will be spawned.

This event is useful for starting a set of programs in a standard
configuration to save you the effort of doing it manually each time:

```lua
local wezterm = require 'wezterm'
local mux = wezterm.mux

-- this is called by the mux server when it starts up.
-- It makes a window split top/bottom
wezterm.on('mux-startup', function()
  local tab, pane, window = mux.spawn_window {}
  pane:split { direction = 'Top' }
end)

return {
  unix_domains = {
    { name = 'unix' },
  },
}
```

See also:
* [wezterm.mux](../wezterm.mux/index.md)
