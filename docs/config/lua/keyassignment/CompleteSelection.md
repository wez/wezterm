# `CompleteSelection`

Completes an active text selection process; the selection range is
marked closed and then the selected text is copied as though the
`Copy` action was executed.

{{since('20210203-095643-70a364eb')}}

`CompleteSelection` now requires a destination parameter to specify
which clipboard buffer the selection will populate; the copy action
is now equivalent to [CopyTo](CopyTo.md).

```lua
config.mouse_bindings = {
  -- Change the default click behavior so that it only selects
  -- text and doesn't open hyperlinks, and that it populates
  -- the Clipboard rather the PrimarySelection which is part
  -- of the default assignment for a left mouse click.
  {
    event = { Up = { streak = 1, button = 'Left' } },
    mods = 'NONE',
    action = wezterm.action.CompleteSelection 'Clipboard',
  },
}
```
