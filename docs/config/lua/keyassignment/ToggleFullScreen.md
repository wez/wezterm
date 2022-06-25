# ToggleFullScreen

Toggles full screen mode for the current window.  (But see:
<https://github.com/wez/wezterm/issues/177>)

```lua
local wezterm = require 'wezterm'

return {
  keys = {
    {key="n", mods="SHIFT|CTRL", action=wezterm.action.ToggleFullScreen},
  }
}
```


