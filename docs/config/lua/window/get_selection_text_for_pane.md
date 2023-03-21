# `window:get_selection_text_for_pane(pane)`

{{since('20210404-112810-b63a949d')}}

Returns the text that is currently selected within the specified pane within
the specified window.  This is the same text that would be copied to the
clipboard if the [CopyTo](../keyassignment/CopyTo.md) action were to be
performed.

Why isn't this simply a method of the `pane` object?  The reason is that the
selection is an attribute of the containing window, and a given pane can
potentially be mapped into multiple windows.

This example logs the current selection when a CTRL+SHIFT+E is pressed:

```lua
local wezterm = require 'wezterm'

wezterm.on('log-selection', function(window, pane)
  local sel = window:get_selection_text_for_pane(pane)
  wezterm.log_info('selection is: ' .. sel)
end)

return {
  keys = {
    {
      key = 'E',
      mods = 'CTRL',
      action = wezterm.action.EmitEvent 'log-selection',
    },
  },
}
```
