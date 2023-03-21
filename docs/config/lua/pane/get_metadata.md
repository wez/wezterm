# `pane:get_metadata()`

{{since('20220903-194523-3bb1ed61')}}

Returns metadata about a pane. The return value depends on the instance of the
underlying pane. If the pane doesn't support this method, `nil` will be returned.
Otherwise, the value is a lua table with the metadata contained in table fields.

To consume this value, it is recommend to use logic like this to obtain a table
value even if the pane doesn't support this method:

```lua
local meta = pane:get_metadata() or {}
```

The following metadata keys may be present:

## password_input

A boolean value that is populated only for local panes.
It is set to true if it appears as though the local PTY is
configured for password entry (local echo disabled, canonical
input mode enabled).

This example demonstrates how to change the color scheme
to exaggerate when a password is being input:

```lua
local wezterm = require 'wezterm'

wezterm.on('update-status', function(window, pane)
  local meta = pane:get_metadata() or {}
  local overrides = window:get_config_overrides() or {}
  if meta.password_input then
    overrides.color_scheme = 'Red Alert'
  else
    overrides.color_scheme = nil
  end
  window:set_config_overrides(overrides)
end)

return {}
```

## is_tardy

A boolean value that is populated only for multiplexer client panes.
It is set to true if wezterm is waiting for a response from the multiplexer
server.

This can be used in conjunction with `since_last_response_ms` below.

## since_last_response_ms

An integer value that is populated only for multiplexer client panes.
It is set to the number of elapsed milliseconds since the most recent
response from the multiplexer server.

This example shows how to put mux latency information into the status area:

```lua
local wezterm = require 'wezterm'

wezterm.on('update-status', function(window, pane)
  local meta = pane:get_metadata() or {}
  if meta.is_tardy then
    local secs = meta.since_last_response_ms / 1000.0
    window:set_right_status(string.format('tardy: %5.1fs⏳', secs))
  end
end)

return {}
```

