# PasteFrom(source)

Paste the specified clipboard to the current pane.

This is only really meaningful on X11 systems that have multiple clipboards.

Possible values for source are:

* `Clipboard` - paste from the system clipboard
* `PrimarySelection` - paste from the primary selection buffer

See also [Paste](Paste.md).

```lua
local wezterm = require 'wezterm';
return {
  keys = {
    -- paste from the clipboard
    {key="V", mods="CTRL", action=PasteFrom="Clipboard"},

    -- paste from the primary selection
    {key="V", mods="CTRL", action=PasteFrom="PrimarySelection"},
  },
}
```

