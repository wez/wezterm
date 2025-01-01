# `ClearSelection`

{{since('20220624-141144-bd1b7c5d')}}

Clears the selection in the current pane.

This example shows how to rebind `CTRL-C` to copy to the clipboard
when there is a selection present (clearing it afterwards) or sending
CTRL-C to the terminal when there is no selection:

```lua
local wezterm = require 'wezterm'
local act = wezterm.action

config.keys = {
  {
    key = 'c',
    mods = 'CTRL',
    action = wezterm.action_callback(function(window, pane)
      local has_selection = window:get_selection_text_for_pane(pane) ~= ''
      if has_selection then
        window:perform_action(act.CopyTo 'ClipboardAndPrimarySelection', pane)

        window:perform_action(act.ClearSelection, pane)
      else
        window:perform_action(act.SendKey { key = 'c', mods = 'CTRL' }, pane)
      end
    end),
  },
}
```
