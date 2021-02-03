# SelectTextAtMouseCursor

Initiates selection of text at the current mouse cursor position.
The mode argument can be one of `Cell`, `Word` or `Line` to control
the scope of the selection.

*Since: 20210203-095643-70a364eb*

The mode argument can be `SemanticZone` which causes the selection
to take the surrounding semantic zone.

In this example, the triple-left-click mouse action is set to
automatically select the entire command output when clicking
on any character withing that region:

```lua
return {
  mouse_bindings = {
    { event={Down={streak=3, button="Left"}},
      action={SelectTextAtMouseCursor="SemanticZone"},
      mods="NONE"
    },
  },
}
```

[See Shell Integration docs](../../../shell-integration.md) for more details on
how to set up your shell to define semantic zones.

