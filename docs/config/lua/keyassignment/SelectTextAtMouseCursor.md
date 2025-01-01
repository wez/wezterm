# `SelectTextAtMouseCursor`

Initiates selection of text at the current mouse cursor position.
The mode argument can be one of `Cell`, `Word` or `Line` to control
the scope of the selection.

{{since('20210203-095643-70a364eb')}}

The mode argument can be `SemanticZone` which causes the selection
to take the surrounding semantic zone.

In this example, the triple-left-click mouse action is set to
automatically select the entire command output when clicking
on any character within that region:

```lua
config.mouse_bindings = {
  {
    event = { Down = { streak = 3, button = 'Left' } },
    action = wezterm.action.SelectTextAtMouseCursor 'SemanticZone',
    mods = 'NONE',
  },
}
```

[See Shell Integration docs](../../../shell-integration.md) for more details on
how to set up your shell to define semantic zones.

{{since('20220624-141144-bd1b7c5d')}}

The mode argument can also be `"Block"` to enable a rectangular block selection.
