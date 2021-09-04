# SendKey

*Since: nightly builds only*

Send the specified key press to the current pane.  This is useful to rebind
the effect of a key combination.

Note that this rebinding effect only applies to the input that is about to be
sent to the pane; it doesn't get re-evaluated against the key assignments
you've configured in wezterm again.

For example, macOS users often prefer to rebind `Option+LeftArrow` and
`Option+RightArrow` to match the behavior of Terminal.app, where those key
sequences are remapped to `ALT-b` and `ALT-f` which generally causes the
the cursor to move backwards or forwards by one word in most common unix
shells and applications.

The following configuration achieves that same effect:

```lua
return {
  keys = {
     -- Rebind OPT-Left, OPT-Right as ALT-b, ALT-f respectively to match Terminal.app behavior
     {key="LeftArrow", mods="OPT", action={SendKey={key="b", mods="ALT"}}},
     {key="RightArrow", mods="OPT", action={SendKey={key="f", mods="ALT"}}},
  }
}
```

See also [Multiple](Multiple.md) for combining multiple actions in a single press.
