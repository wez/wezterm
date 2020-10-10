# ExtendSelectionToMouseCursor

Extends the current text selection to the current mouse cursor position.
The mode argument can be one of `Cell`, `Word` or `Line` to control
the scope of the selection.

It is also possible to leave the mode unspecified like this:

```lua
return {
  mouse_bindings = {
    {
      event={Up={streak=1, button="Left"}},
      mods="SHIFT",
      -- Note that there is no `wezterm.action` here
      action={ExtendSelectionToMouseCursor={}},
    },
  }
}
```

when unspecified, wezterm will use a default mode which at the time
of writing is `Cell`, but in a future release may be context sensitive
based on recent actions.


