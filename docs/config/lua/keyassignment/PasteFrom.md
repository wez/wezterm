# `PasteFrom(source)`

Paste the specified clipboard to the current pane.

This is only really meaningful on X11 and some Wayland systems that have multiple clipboards.

Possible values for source are:

* `Clipboard` - paste from the system clipboard
* `PrimarySelection` - paste from the primary selection buffer

```lua
local wezterm = require 'wezterm'
local act = wezterm.action

config.keys = {
  -- paste from the clipboard
  { key = 'V', mods = 'CTRL', action = act.PasteFrom 'Clipboard' },

  -- paste from the primary selection
  { key = 'V', mods = 'CTRL', action = act.PasteFrom 'PrimarySelection' },
}
```

{{since('20220319-142410-0fcdea07')}}

`PrimarySelection` is now also supported on Wayland systems that support [primary-selection-unstable-v1](https://wayland.app/protocols/primary-selection-unstable-v1) or the older Gtk primary selection protocol.
