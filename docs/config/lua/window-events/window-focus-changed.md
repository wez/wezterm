# `window-focus-changed`

{{since('20221119-145034-49b9839f')}}

The `window-focus-changed` event is emitted when the focus state for a window
is changed.

This event is fire-and-forget from the perspective of wezterm; it fires the
event to advise of the config change, but has no other expectations.

The first event parameter is a [`window` object](../window/index.md) that
represents the gui window.

The second event parameter is a [`pane` object](../pane/index.md) that
represents the active pane in that window.

```lua
local wezterm = require 'wezterm'

wezterm.on('window-focus-changed', function(window, pane)
  wezterm.log_info(
    'the focus state of ',
    window:window_id(),
    ' changed to ',
    window:is_focused()
  )
end)
```

