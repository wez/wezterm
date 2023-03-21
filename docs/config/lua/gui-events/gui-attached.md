# `gui-attached`

{{since('20230320-124340-559cb7b0')}}

This event is triggered when the GUI is starting up after attaching the
selected domain.  For example, when you use `wezterm connect DOMAIN` or
`wezterm start --domain DOMAIN` to start the GUI, the `gui-attached` event will
be triggered and passed the [MuxDomain](../MuxDomain/index.md) object
associated with `DOMAIN`.  In cases where you don't specify the domain, the
default domain will be passed instead.

This event fires after the [gui-startup](gui-startup.md) event.

Note that the `gui-startup` event does not fire when invoking `wezterm connect
DOMAIN` or `wezterm start --domain DOMAIN --attach`.

You can use this opportunity to take whatever action suits your purpose; some
users like to maximize all of their windows on startup, and this event would
allow you do that:

```lua
local wezterm = require 'wezterm'
local mux = wezterm.mux

wezterm.on('gui-attached', function(domain)
  -- maximize all displayed windows on startup
  local workspace = mux.get_active_workspace()
  for _, window in ipairs(mux.all_windows()) do
    if window:get_workspace() == workspace then
      window:gui_window():maximize()
    end
  end
end)

local config = wezterm.config_builder()

return config
```

See also: [gui-startup](gui-startup.md).
