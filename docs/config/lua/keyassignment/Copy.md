# Copy

Copy the selection to the clipboard.

*since: 20210203-095643-70a364eb*

This action is considered to be deprecated and will be removed in
a future release; please use [CopyTo](CopyTo.md) instead.

*Since: nightly builds only*

This action has been removed. Please use [CopyTo](CopyTo.md) instead.

## Example


```lua
local wezterm = require 'wezterm'
return {
  keys = {
    { key = 'C', mods = 'CTRL', action = wezterm.action.Copy },
  },
}
```

