# `gui-startup`

*Since: nightly builds only*

The `gui-startup` event is emitted once when the GUI server is starting up
when running the `wezterm start` subcommand.

It is triggered before any default program is started.

If no explicit program was passed to `wezterm start`, and if the
`gui-startup` event causes any panes to be created then those will take
precedence over the default program configuration and no additional default
program will be spawned.

This event is useful for starting a set of programs in a standard
configuration to save you the effort of doing it manually each time:

```lua
local wezterm = require 'wezterm'
local mux = wezterm.mux

wezterm.on("gui-startup", function()
  local tab, pane, window = mux.spawn_window{}
  mux.split_pane(pane, {size=0.3})
  mux.split_pane(pane, {size=0.5})
end)

return {}
```

