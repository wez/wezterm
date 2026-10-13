# `IncreaseFontSizeByUnit`

Increases the font size of the current window by the specified unit. Unit can be
point (pt), pixel (px), percentage (%) and cell (cell).

```lua
config.keys = {
  -- by point
  { key = '=', mods = 'CTRL', action = wezterm.action.IncreaseFontSizeByUnit("1 pt") },

  -- by pixels
  { key = '=', mods = 'CTRL', action = wezterm.action.IncreaseFontSizeByUnit("3 px") },

  -- by percentage
  { key = '=', mods = 'CTRL', action = wezterm.action.IncreaseFontSizeByUnit("15 %") },

  -- by cells
  { key = '=', mods = 'CTRL', action = wezterm.action.IncreaseFontSizeByUnit("0.5 cell") },
}
```

See also [adjust_window_size_when_changing_font_size](../config/adjust_window_size_when_changing_font_size.md)
and [IncreaseFontSize](./IncreaseFontSize.md)
