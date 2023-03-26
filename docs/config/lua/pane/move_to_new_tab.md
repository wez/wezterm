# `pane:move_to_new_tab()`

{{since('nightly')}}

Creates a new tab in the window that contains `pane`, and moves `pane` into that tab.

Returns the newly created [MuxTab](../MuxTab/index.md) object, and the
[MuxWindow](../mux-window/index.md) object that contains it:

```lua
config.keys = {
  {
    key = '!',
    mods = 'LEADER | SHIFT',
    action = callback(function(win, pane)
      local tab, window = pane:move_to_new_tab()
    end),
  },
}
```

See also [pane:move_to_new_window()](move_to_new_window.md),
[wezterm cli move-pane-to-new-tab](../../../cli/cli/move-pane-to-new-tab.md).
