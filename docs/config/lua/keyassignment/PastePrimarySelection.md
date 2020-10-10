# PastePrimarySelection

X11: Paste the Primary Selection to the current tab.
On other systems, this behaves identically to `Paste`.

```lua
local wezterm = require 'wezterm';
return {
  keys = {
    {key="v", mods="SHIFT|CTRL", action="PastePrimarySelection"},
  }
}
```


