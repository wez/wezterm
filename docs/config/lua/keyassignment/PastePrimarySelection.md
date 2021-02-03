# PastePrimarySelection

X11: Paste the Primary Selection to the current tab.
On other systems, this behaves identically to [Paste](Paste.md).

*since: 20210203-095643-70a364eb*

This action is considered to be deprecated and will be removed in
a future release; please use [PasteFrom](PasteFrom.md) instead.

## Example

```lua
local wezterm = require 'wezterm';
return {
  keys = {
    {key="v", mods="SHIFT|CTRL", action="PastePrimarySelection"},
  },

  -- Middle mouse button pastes the primary selection.
  mouse_bindings = {
    {
      event={Up={streak=1, button="Middle"}},
      mods="NONE",
      action="PastePrimarySelection",
    },
  }
}
```

