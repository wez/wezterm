# `ScrollByLine`

{{since('20210314-114017-04b7cedd')}}

Adjusts the scroll position by the number of lines specified by the argument.
Negative values scroll upwards, while positive values scroll downwards.

```lua
local act = wezterm.action

config.keys = {
  { key = 'UpArrow', mods = 'SHIFT', action = act.ScrollByLine(-1) },
  { key = 'DownArrow', mods = 'SHIFT', action = act.ScrollByLine(1) },
}
```

