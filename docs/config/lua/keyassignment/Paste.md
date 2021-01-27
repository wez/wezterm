# Paste

Paste the clipboard to the current pane.

On X11 systems that have multiple clipboards, the
[default_clipboard_paste_source](../config/default_clipboard_paste_source.md)
option specifies which source to use.

See also [PastePrimarySelection](PastePrimarySelection.md) and [PasteFrom](PasteFrom.md).

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


