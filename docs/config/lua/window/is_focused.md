# wezterm:is_focused()

{{since('20221119-145034-49b9839f')}}

Returns `true` if the window has focus.

The `update-status` event is fired when the focus state changes.

This example changes the color scheme based on the focus state:

```lua
local wezterm = require 'wezterm'

wezterm.on('update-status', function(window, pane)
  local overrides = window:get_config_overrides() or {}
  if window:is_focused() then
    overrides.color_scheme = 'nordfox'
  else
    overrides.color_scheme = 'nightfox'
  end
  window:set_config_overrides(overrides)
end)

return {}
```

