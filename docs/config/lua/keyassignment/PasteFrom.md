# PasteFrom(source)

Paste the specified clipboard to the current pane.

This is only really meaningful on X11 and some Wayland systems that have multiple clipboards.

Possible values for source are:

* `Clipboard` - paste from the system clipboard
* `PrimarySelection` - paste from the primary selection buffer

See also [Paste](Paste.md).

```lua
local wezterm = require 'wezterm';
return {
  keys = {
    -- paste from the clipboard
    {key="V", mods="CTRL", action=wezterm.action{PasteFrom="Clipboard"}},

    -- paste from the primary selection
    {key="V", mods="CTRL", action=wezterm.action{PasteFrom="PrimarySelection"}},
  },
}
```

*Since: nightly builds only*

`PrimarySelection` is now also supported on Wayland systems that support [primary-selection-unstable-v1](https://wayland.app/protocols/primary-selection-unstable-v1) or the older Gtk primary selection protocol.
