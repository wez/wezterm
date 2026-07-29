---
tags:
  - appearance
---
# `dim_tint_factor = 1.0`

Controls the brightness of text rendered with the dim/faint SGR attribute
(SGR 2, also known as `Intensity::Half`).

The value is a multiplier in the range `0.0`–`1.0` applied to the RGB
components of the foreground color when a cell has the dim attribute set:

* `1.0` (the default) — no additional color change beyond the lighter font
  weight that WezTerm already selects for dim text via `font_rules`.
* `0.5` — halves the brightness of dim text.
* `0.0` — renders dim text as black (or the darkest possible shade).

This multiplier is applied **in addition to** the font-weight substitution,
so both effects are active simultaneously when the value is less than `1.0`.

```lua
config.dim_tint_factor = 0.5
```

Note: the multiplier is applied in the linear light color space, so the
perceptual effect is consistent across different base colors.
