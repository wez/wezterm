# `wezterm.color.parse(string)`

{{since('20220807-113146-c2fee766')}}

Parses the passed color and returns a [Color
object](../color/index.md).  `Color` objects evaluate as strings but
have a number of methods that allow transforming and comparing
colors.

```
> wezterm.color.parse("black")
#000000
```

This example picks a foreground color, computes its complement in
the "artist's color wheel" to produce a purple color and then
darkens it to use it as a background color:

```lua
local wezterm = require 'wezterm'

local fg = wezterm.color.parse 'yellow'
local bg = fg:complement_ryb():darken(0.2)

return {
  colors = {
    foreground = fg,
    background = bg,
  },
}
```

