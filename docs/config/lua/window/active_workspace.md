# `window:active_workspace()`

{{since('20220319-142410-0fcdea07')}}

Returns the name of the active workspace.

This example demonstrates using the launcher menu to select and create workspaces,
and how the workspace can be shown in the right status area.

```lua
local wezterm = require 'wezterm'

wezterm.on('update-right-status', function(window, pane)
  window:set_right_status(window:active_workspace())
end)

return {
  keys = {
    {
      key = '9',
      mods = 'ALT',
      action = wezterm.action.ShowLauncherArgs { flags = 'FUZZY|WORKSPACES' },
    },
  },
}
```
