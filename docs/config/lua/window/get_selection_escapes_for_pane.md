# `window:get_selection_escapes_for_pane(pane)`

*Since: nightly builds only*

Returns the text that is currently selected within the specified pane within
the specified window formatted with the escape sequences necessary to reproduce
the same colors and styling .

This is the same text that
[window:get_selection_text_for_pane()](get_selection_text_for_pane.md) would
return, except that it includes escape sequences.

This example copies the current selection + escapes to the clipboard when
`CTRL+SHIFT+E` is pressed:

```lua
local wezterm = require 'wezterm'

return {
  keys = {
    {key="E", mods="CTRL", action=wezterm.action_callback(function(window, pane)
      local ansi = window:get_selection_escapes_for_pane(pane);
      window:copy_to_clipboard(ansi)
    end)}
  }
}
```

See also: [window:copy_to_clipboard()](copy_to_clipboard.md).
