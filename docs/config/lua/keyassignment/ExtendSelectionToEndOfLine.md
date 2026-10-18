# `ExtendSelectionToEndOfLine`

{{since('nightly')}}

Extends the end of the current text selection to the end of the logical line
on which the selection ends. If the line is wrapped, the selection extends to
the end of its last wrapped row. If there is no current selection, this action
does nothing.

This is intended to be combined with
[SelectTextAtMouseCursor](SelectTextAtMouseCursor.md) using
[Multiple](Multiple.md). For example, to make a triple click select from the
word under the mouse pointer to the end of the line, as in Konsole:

```lua
local act = wezterm.action

config.mouse_bindings = {
  {
    event = { Down = { streak = 3, button = 'Left' } },
    action = act.Multiple {
      act.SelectTextAtMouseCursor 'Word',
      act.ExtendSelectionToEndOfLine,
    },
  },
}
```

When combined with `Word`, the selection starts at the beginning of the word
under the pointer, not at the exact cell that was clicked.

Dragging after the triple click switches to whole-line selection, because of
the default `Drag` with `streak = 3` binding for `ExtendSelectionToMouseCursor 'Line'`.

This action does not special-case rectangular (block) selections.

Note that a `Cell` selection does not exist until the mouse moves, so use
`Word` (or `Line`/`SemanticZone`) as the initial selection.
