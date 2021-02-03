# Paste

Paste the clipboard to the current pane.

*since: 20210203-095643-70a364eb*

This action is considered to be deprecated and will be removed in
a future release; please use [PasteFrom](PasteFrom.md) instead.

## Example

```lua
local wezterm = require 'wezterm';
return {
  keys = {
    {key="v", mods="SHIFT|CTRL", action="Paste"},
  },

  -- Middle mouse button pastes the clipboard.
  -- Note that this is the default so you needn't copy this.
  mouse_bindings = {
    {
      event={Up={streak=1, button="Middle"}},
      mods="NONE",
      action="Paste",
    },
  }
}
```


