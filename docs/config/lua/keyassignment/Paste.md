# Paste

Paste the clipboard to the current tab.  On X11 systems, this copies from the
Clipboard rather than the Primary Selection.  See also `PastePrimarySelection`.

```lua
local wezterm = require 'wezterm';
return {
  keys = {
    {key="v", mods="SHIFT|CTRL", action="Paste"},
  }
}
```


