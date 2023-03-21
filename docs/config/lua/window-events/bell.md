# `bell`

{{since('20211204-082213-a66c61ee9')}}

The `bell` event is emitted when the ASCII BEL sequence is emitted to
a pane in the window.

Defining an event handler doesn't alter wezterm's handling of the bell;
the event supplements it and allows you to take additional action over
the configured behavior.

The first event parameter is a [`window` object](../window/index.md) that
represents the gui window.

The second event parameter is a [`pane` object](../pane/index.md) that
represents the pane in which the bell was rung, which may not be active
pane--it could be in an unfocused pane or tab..

```lua
local wezterm = require 'wezterm'

wezterm.on('bell', function(window, pane)
  wezterm.log_info('the bell was rung in pane ' .. pane:pane_id() .. '!')
end)

return {}
```

See also [audible_bell](../config/audible_bell.md) and [visual_bell](../config/visual_bell.md).
