# Workspaces / Sessions

If you are familiar with tmux then you may have used its session functionality
to manage separate groups of related tmux windows (which map to tabs in wezterm).

wezterm doesn't have an exact match to tmux sessions, but does have a similar
concept known as *Workspaces*.

Every [MuxWindow](../config/lua/mux-window/index.md) is associated with a
workspace, which is just a label.

The wezterm GUI is focused on the active workspace, which means that it will
present a GUI window for each MuxWindow that is present in that workspace.

You can spawn windows into differently named workspaces and they won't become
visible until you set the active workspace to that name.

When switching the active workspace, wezterm will swap the contents of the
GUI windows with the MuxWindows that belong to the now-focused workspace.

The following key assignments are helpful when working with workspaces:

* [SwitchToWorkspace](../config/lua/keyassignment/SwitchToWorkspace.md)
* [SwitchWorkspaceRelative](../config/lua/keyassignment/SwitchWorkspaceRelative.md)
* Various key assignments or functions that spawn windows also allow specifying
  the workspace name to be used
* [ShowLauncher](../config/lua/keyassignment/ShowLauncher.md) and
  [ShowLauncherArgs](../config/lua/keyassignment/ShowLauncherArgs.md) will list
  the current set of workspaces and allow switching between them

You can pre-define a layout of windows/tabs/panes in specific workspaces by
using these events:

* [gui-startup](../config/lua/gui-events/gui-startup.md)
* [mux-startup](../config/lua/mux-events/mux-startup.md)

