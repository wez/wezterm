# CopyTo(destination)

Copy the selection to the specified clipboard buffer.

Possible values for destination are:

* `Clipboard` - copy the text to the system clipboard.
* `PrimarySelection` - Copy the test to the primary selection buffer (applicable to X11 systems only)
* `ClipboardAndPrimarySelection` - Copy to both the clipboard and the primary selection.

```lua
local wezterm = require 'wezterm';
return {
  keys = {
    {key="C", mods="CTRL", action=wezterm.action{CopyTo="ClipboardAndPrimarySelection"}},
  }
}
```

