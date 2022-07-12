## `color:triad()`

*Since: nightly builds only*

Returns the other two colors that form a triad. The other colors
are at +/- 120 degrees in the HSL color wheel.

```lua
local a, b = wezterm:color.parse("yellow"):triad()
```


