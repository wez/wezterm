# `ExtendSelectionToMouseCursor`

Extends the current text selection to the current mouse cursor position.
The mode argument can be one of `Cell`, `Word` or `Line` to control
the scope of the selection.

{{since('20220624-141144-bd1b7c5d')}}

The mode argument can also be `"Block"` to enable a rectangular block selection.

```lua
config.mouse_bindings = {
  {
    event = { Up = { streak = 1, button = 'Left' } },
    mods = 'SHIFT',
    action = wezterm.action.ExtendSelectionToMouseCursor 'Word',
  },
}
```

