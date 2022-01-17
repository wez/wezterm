# SwitchWorkspaceRelative

*Since: nightly builds only*

Switch to the workspace relative to the current workspace.  Workspaces are ordered
lexicographically based on their names.

The argument value specifies an offset. eg: `-1` switches to the workspace
immediately prior to the current workspace, while `1` switches to the workspace
immediately following the current workspace.

This example binds CTRL-N and CTRL-P to move forwards, backwards through workspaces.
It shows the active workspace in the title bar.  The launcher menu can be used
to create workspaces.

```lua
local wezterm = require 'wezterm'

wezterm.on("update-right-status", function(window, pane)
  window:set_right_status(window:active_workspace())
end)

return {
  keys = {
    {key="9", mods="ALT", action=wezterm.action{ShowLauncherArgs={flags="FUZZY|WORKSPACES"}}},
    {key="n", mods="CTRL", action=wezterm.action{SwitchWorkspaceRelative=1}},
    {key="p", mods="CTRL", action=wezterm.action{SwitchWorkspaceRelative=-1}},
  },
}
```

