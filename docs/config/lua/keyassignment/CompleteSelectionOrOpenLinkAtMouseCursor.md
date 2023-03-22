# `CompleteSelectionOrOpenLinkAtMouseCursor`

If a selection is in progress, acts as though `CompleteSelection` was
triggered.  Otherwise acts as though `OpenLinkAtMouseCursor` was
triggered.


{{since('20210203-095643-70a364eb')}}

`CompleteSelectionOrOpenLinkAtMouseCursor` now requires a destination parameter to specify
which clipboard buffer the selection will populate. The copy action
is now equivalent to [CopyTo](CopyTo.md).

```lua
config.mouse_bindings = {
  -- Change the default click behavior so that it populates
  -- the Clipboard rather the PrimarySelection.
  {
    event = { Up = { streak = 1, button = 'Left' } },
    mods = 'NONE',
    action = wezterm.action.CompleteSelectionOrOpenLinkAtMouseCursor 'Clipboard',
  },
}
```
