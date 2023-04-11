# `pane:move_to_new_window([WORKSPACE])`

{{since('20230326-111934-3666303c')}}

Creates a window and moves `pane` into that window.

The *WORKSPACE* parameter is optional; if specified, it will be used
as the name of the workspace that should be associated with the new
window.  Otherwise, the current active workspace will be used.

Returns the newly created [MuxTab](../MuxTab/index.md) object, and the
newly created [MuxWindow](../mux-window/index.md) object.

```lua
config.keys = {
  {
    key = '!',
    mods = 'LEADER | SHIFT',
    action = wezterm.action_callback(function(win, pane)
      local tab, window = pane:move_to_new_window()
    end),
  },
}
```

See also [pane:move_to_new_window()](move_to_new_window.md),
[wezterm cli move-pane-to-new-tab](../../../cli/cli/move-pane-to-new-tab.md).


