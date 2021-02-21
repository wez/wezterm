# ResetFontAndWindowSize

*Since: nightly builds only*

Reset both the font size and the terminal dimensions for the current window to
the values specified by your `font`, `initial_rows`, and `initial_cols` configuration.

```lua
local wezterm = require 'wezterm';
return {
  keys = {
    {key="0", mods="CTRL", action="ResetFontAndWindowSize"},
  }
}
```


