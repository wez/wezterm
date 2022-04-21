# ClearSelection

*Since: nightly builds only*

Clears the selection in the current pane.

This example shows how to rebind `CTRL-C` to copy to the clipboard
when there is a selection present (clearing it afterwards) or sending
CTRL-C to the terminal when there is no selection:

```lua
local wezterm = require 'wezterm'

return {
  keys = {
    {
      key="c",
      mods="CTRL",
      action = wezterm.action_callback(function(window, pane)
        local has_selection = window:get_selection_text_for_pane(pane) ~= ""
        if has_selection then
          window:perform_action(
            wezterm.action{CopyTo="ClipboardAndPrimarySelection"},
            pane)

          window:perform_action("ClearSelection", pane)
        else
          window:perform_action(
            wezterm.action{SendKey={key="c", mods="CTRL"}},
            pane)
        end
      end)
    }
  }
}
```
