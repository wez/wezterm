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

## `color:complement_ryb()`

*Since: nightly builds only*

Returns the complement of the color using the [RYB color
model](https://en.wikipedia.org/wiki/RYB_color_model), which more closely
matches how artists think of mixing colors.

The complement is computed by converting to HSL, converting the
hue angle to the equivalent RYB angle, rotating by 180 degrees and
and then converting back to RGBA.

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
local a, b, c = wezterm:color.parse("yellow"):square()
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

180 degrees gives the complementary color.
Three colors separated by 120 degrees form the triad.
Four colors separated by 90 degrees form the square.

## `color:adjust_hue_fixed_ryb(degrees)`

*Since: nightly builds only*

Adjust the hue angle using the [RYB color model](https://en.wikipedia.org/wiki/RYB_color_model), which more closely
matches how artists think of mixing colors, by the specified number of degrees.

180 degrees gives the complementary color.
Three colors separated by 120 degrees form the triad.
Four colors separated by 90 degrees form the square.

## `color:hsla()`

*Since: nightly builds only*

Converts the color to the HSL colorspace and returns those values + alpha:

```lua
local h, s, l, a = color:hsla()
```

### `color:laba()`

*Since: nightly builds only*

Converts the color to the LAB colorspace and returns those values + alpha:

```lua
local l, a, b, alpha = color:laba()
```

### `color:contrast_ratio(color)`

*Since: nightly builds only*

Computes the contrast ratio between the two colors.

```lua
> wezterm.color.parse("red"):contrast_ratio(wezterm.color.parse("yellow"))
1
> wezterm.color.parse("red"):contrast_ratio(wezterm.color.parse("navy"))
1.8273614734023298
```

The contrast ratio is computed by first converting to HSL, taking the L
components, and diving the lighter one by the darker one.

A contrast ratio of 1 means no contrast.

The maximum possible contrast ratio is 21:

```lua
> wezterm.color.parse("black"):contrast_ratio(wezterm.color.parse("white"))
21
```

### `color:delta_e(color)`

*Since: nightly builds only*

Computes the CIEDE2000 DeltaE value for the two colors.
A value:

* <= 1.0: Not perceptible by the human eye
* 1-2: Perceptible through close observation
* 2-10: Perceptible at a glance
* 11-49: Colors are more similar than the opposite
* 100: Colors are exactly the opposite


