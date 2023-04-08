---
tags:
  - font
---
# `cell_width = 1.0`

{{since('20220624-141144-bd1b7c5d')}}

Scales the computed cell width to adjust the spacing between successive cells
of text.

If possible, you should prefer to specify the `stretch` parameter when
selecting a font using [wezterm.font](../wezterm/font.md) or
[wezterm.font_with_fallback](../wezterm/font_with_fallback.md) as that will
generally look better and have fewer undesirable side effects.

If your preferred font doesn't have variations with different stretches, or
if the font spacing still doesn't look right to you, then `cell_width` gives
you a simple way to influence the spacing.

The default cell width is indirectly controlled by the [font](font.md) and
[font_size](font_size.md) configuration options; the selected font and font
size controls the height of the font, while the font designer controls the
aspect ratio of the glyphs in the font.  The base font (the first font resolved
from your `font` configuration) defines the cell metrics for the terminal
display grid, and those metrics are then used to place glyphs, regardless of
which fallback font might be resolved for a given glyph.

If you feel that your chosen font feels too horizontally cramped then you can
set `cell_width = 1.2` to increase the horizontal spacing by 20%.  Conversely,
setting `cell_width = 0.9` will decrease the horizontal spacing by 10%.

This option doesn't adjust the rasterized width of glyphs, it just changes what
wezterm considers to be the cell boundaries. When rendering monospace, wezterm
advances by the cell width to place successive glyphs.

If you set the scale less than `1.0` then the glyphs won't be truncated or
squished, but will render over the top of each other.  Conversely, if you set
the scale to greater than `1.0`, the glyphs won't be stretched but will render
further apart from each other.

Changing `cell_width` doesn't adjust the positioning of the glyph within the
cell: it remains at its usual x-position.  It is *not* centered within the
adjusted space.

Changing `cell_width` may have undesirable consequences, especially for fonts
that use ligatures: depending on the font, you may find that some ligatured
sequences are misaligned or render strangely. This is not a bug: the font is
designed to be rendered with a `cell_width = 1.0`, so running with a different
value will have this sort of side effect.

See also: [line_height](line_height.md)

