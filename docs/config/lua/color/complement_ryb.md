# `color:complement_ryb()`

{{since('20220807-113146-c2fee766')}}

Returns the complement of the color using the [RYB color
model](https://en.wikipedia.org/wiki/RYB_color_model), which more closely
matches how artists think of mixing colors.

The complement is computed by converting to HSL, converting the
hue angle to the equivalent RYB angle, rotating by 180 degrees and
and then converting back to RGBA.

See also: [color:complement()](complement.md).
