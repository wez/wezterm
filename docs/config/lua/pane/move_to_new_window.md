# `pane:move_to_new_window([WORKSPACE])`

{{since('20230326-111934-3666303c')}}

Creates a window and moves `pane` into that window.

The *WORKSPACE* parameter is optional; if specified, it will be used
as the name of the workspace that should be associated with the new
window.  Otherwise, the current active workspace will be used.

This action fails if the pane is alone in its tab, and the window has a single tab.
{{since('nightly', inline=True)}}

Returns the newly created [MuxTab](../MuxTab/index.md) object, and the
newly created [MuxWindow](../mux-window/index.md) object.

```lua
config.keys = {
  {
    key = '!',
    mods = 'LEADER | SHIFT',
    action = wezterm.action_callback(function(win, pane)
      wezterm.log_info "Moving pane to new window.."
      if #win:mux_window():tabs() == 1 and #pane:tab():panes() == 1 then
        wezterm.log_info "Only one pane in this window, nothing to do!"
        return
      end
      local tab, mux_window = pane:move_to_new_window()
      -- ...
    end),
  },
}
```

See also [`pane:move_to_new_tab()`](move_to_new_tab.md), [wezterm cli move-pane-to-new-tab](../../../cli/cli/move-pane-to-new-tab.md).
