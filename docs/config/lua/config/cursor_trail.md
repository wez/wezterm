---
tags:
  - appearance
  - text_cursor
---
# `cursor_trail = true`

{{since('nightly')}}

Controls whether animated cursor motion and continuous light-trail effects are enabled.

When enabled, moving the text cursor across the terminal generates a continuous fluid quad connecting the previous and current cursor positions, matching the single-quad physics model pioneered by Kitty.

The animation speed and fade duration can be adjusted using [cursor_trail_decay](cursor_trail_decay.md).

```lua
local wezterm = require 'wezterm'
local config = wezterm.config_builder()

config.cursor_trail = true
config.cursor_trail_decay = 0.30

return config
```
