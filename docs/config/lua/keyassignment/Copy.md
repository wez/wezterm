# Copy

Copy the selection to the clipboard.  On X11 systems, this populates both the
Clipboard and the Primary Selection.

```lua
local wezterm = require 'wezterm';
return {
  keys = {
    {key="c", mods="SHIFT|CTRL", action="Copy"},
  }
}
```


