# Copy

Copy the selection to the clipboard.

The value of the [default_clipboard_copy_destination](../config/default_clipboard_copy_destination.md) configuration option specifies which clipboard buffer is populated.


```lua
local wezterm = require 'wezterm';
return {
  keys = {
    {key="C", mods="CTRL", action="Copy"},
  }
}
```

