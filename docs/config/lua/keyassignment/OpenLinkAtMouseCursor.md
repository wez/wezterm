# `OpenLinkAtMouseCursor`

If the current mouse cursor position is over a cell that contains
a hyperlink, this action causes that link to be opened.

```lua
config.mouse_bindings = {
  -- Shift-click will open the link under the mouse cursor
  {
    event = { Up = { streak = 1, button = 'Left' } },
    mods = 'Shift',
    action = wezterm.action.OpenLinkAtMouseCursor,
  },
}
```
