# `ToggleFullScreen`

Toggles full screen mode for the current window.

```lua
local wezterm = require 'wezterm'

config.keys = {
  {
    key = 'n',
    mods = 'SHIFT|CTRL',
    action = wezterm.action.ToggleFullScreen,
  },
}
```

See also: [native_macos_fullscreen_mode](../config/native_macos_fullscreen_mode.md).

