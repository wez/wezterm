# PastePrimarySelection

X11: Paste the Primary Selection to the current tab.
On other systems, this behaves identically to [Paste](Paste.md).

*since: nightly*

This action is considered to be deprecated; please consider
using either [PasteFrom](PasteFrom.md) or just [Paste](Paste.md)
and adjusting the new [default_clipboard_paste_source](../config/default_clipboard_paste_source.md) configuration option.

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

