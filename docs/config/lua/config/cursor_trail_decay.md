---
tags:
  - appearance
  - text_cursor
---
# `cursor_trail_decay = 0.30`

{{since('nightly')}}

Controls the decay duration in seconds for the [cursor_trail](cursor_trail.md) animation.

The trail decay applies exponential ease-out physics ($1.0 - 2^{-10 \cdot \frac{dt}{\text{decay}}}$). Leading edges quickly advance toward the target cursor destination, while trailing edges smoothly follow over the configured decay interval.

Smaller values result in faster, snappier cursor transitions; larger values produce a longer, more pronounced gliding trail.

Defaults to `0.30` seconds.

```lua
local wezterm = require 'wezterm'
local config = wezterm.config_builder()

config.cursor_trail = true
config.cursor_trail_decay = 0.30

return config
```
