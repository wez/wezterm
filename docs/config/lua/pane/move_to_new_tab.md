# `pane:move_to_new_tab()`

{{since('20230326-111934-3666303c')}}

Creates a new tab in the window that contains `pane`, and moves `pane` into that tab.

This action fails if the pane is alone in its tab.
{{since('nightly', inline=True)}}

Returns the newly created [MuxTab](../MuxTab/index.md) object, and the
[MuxWindow](../mux-window/index.md) object that contains it:

```lua
local wezterm = require"wezterm"

config.keys = {
  {
    key = '!',
    mods = 'LEADER | SHIFT',
    action = wezterm.action_callback(function(_win, pane)
      wezterm.log_info "Moving pane to new tab.."
      if #pane:tab():panes() == 1 then
        wezterm.log_info "Only one pane in this tab, nothing to do!"
        return
      end
      local tab, mux_window = pane:move_to_new_tab()
      -- ...
    end),
  },
}
```

See also [pane:move_to_new_window()](move_to_new_window.md),
[wezterm cli move-pane-to-new-tab](../../../cli/cli/move-pane-to-new-tab.md).
