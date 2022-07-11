# wezterm.color.parse(string)

*Since: nightly builds only*

Parses the passed color and returns an `RgbaColor` object.
`RgbaColor` objects evaluate as strings but have a number of methods
that allow transforming colors.

```
> wezterm.color.parse("black")
#000000
```

This example picks a foreground color, computes its complement
and darkens it to use it as a background color:

```lua
local wezterm = require 'wezterm'

local fg = wezterm.color.parse("yellow")
local bg = fg:complement():darken(0.2)

return {
  colors = {
    foreground = fg,
    background = bg,
  }
}
```

## `color:complement()`

*Since: nightly builds only*

Returns the complement of the color. The complement is computed
by converting to HSL, rotating by 180 degrees and converting back
to RGBA.

## `color:triad()`

*Since: nightly builds only*

Returns the other two colors that form a triad. The other colors
are at +/- 120 degrees in the HSL color wheel.

```lua
local a, b = wezterm:color.parse("yellow"):triad()
```

## `color:square()`

*Since: nightly builds only*

Returns the other three colors that form a square. The other colors
are 90 degrees apart on the HSL color wheel.

```lua
local a, b,  = wezterm:color.parse("yellow"):square()
```

## `color:saturate(factor)`

*Since: nightly builds only*

Scales the color towards the maximum saturation by the provided factor, which
should be in the range `0.0` through `1.0`.

## `color:saturate_fixed(amount)`

*Since: nightly builds only*

Increase the saturation by amount, a value ranging from `0.0` to `1.0`.

## `color:desaturate(factor)`

*Since: nightly builds only*

Scales the color towards the minimum saturation by the provided factor, which
should be in the range `0.0` through `1.0`.

## `color:desaturate_fixed(amount)`

*Since: nightly builds only*

Decrease the saturation by amount, a value ranging from `0.0` to `1.0`.

## `color:lighten(factor)`

*Since: nightly builds only*

Scales the color towards the maximum lightness by the provided factor, which
should be in the range `0.0` through `1.0`.

## `color:lighten_fixed(amount)`

*Since: nightly builds only*

Increase the lightness by amount, a value ranging from `0.0` to `1.0`.

## `color:darken(factor)`

*Since: nightly builds only*

Scales the color towards the minimum lightness by the provided factor, which
should be in the range `0.0` through `1.0`.

## `color:darken_fixed(amount)`

*Since: nightly builds only*

Decrease the lightness by amount, a value ranging from `0.0` to `1.0`.

## `color:adjust_hue_fixed(degrees)`

*Since: nightly builds only*

Adjust the hue angle by the specified number of degrees.
