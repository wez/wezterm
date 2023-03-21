# `window:effective_config()`

{{since('20210314-114017-04b7cedd')}}

Returns a lua table representing the effective configuration for the Window.
The table is in the same format as that used to specify the config in
the `wezterm.lua` file, but represents the fully-populated state of the
configuration, including any CLI or per-window configuration overrides.

Note: changing the config table will NOT change the effective window config;
it is just a copy of that information.

If you want to change the configuration in a window, look at [set_config_overrides](set_config_overrides.md).

This example will log the configured font size when `CTRL-SHIFT-E` is pressed:

```lua
local wezterm = require 'wezterm'

wezterm.on('show-font-size', function(window, pane)
  wezterm.log_error(window:effective_config().font_size)
end)

return {
  keys = {
    {
      key = 'E',
      mods = 'CTRL',
      action = wezterm.action.EmitEvent 'show-font-size',
    },
  },
}
```
