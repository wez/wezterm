---
tags:
  - appearance
  - font
---
## `custom_block_glyphs = true`

{{since('20210314-114017-04b7cedd')}}

When set to `true` (the default), WezTerm will compute its own idea of what the glyphs
in the following unicode ranges should be, instead of using glyphs resolved from a font.

Ideally this option wouldn't exist, but it is present to work around a [hinting issue in freetype](https://gitlab.freedesktop.org/freetype/freetype/-/issues/761).

|Block|What|Since|
|-----|----|-----|
|[U2500](https://www.unicode.org/charts/PDF/U2580.pdf)|Box Drawing|*20210814-124438-54e29167*|
|[U2580](https://www.unicode.org/charts/PDF/U2580.pdf)|unicode block elements|*20210314-114017-04b7cedd*|
|[U1FB00](https://www.unicode.org/charts/PDF/U1FB00.pdf)|Symbols for Legacy Computing (Sextants and Smooth mosaic graphics)|*20210814-124438-54e29167*|
|[U1CC00](https://www.unicode.org/charts/PDF/U1CC00.pdf)|Symbols for Legacy Computing Supplement (Block mosaic terminal graphic characters)|*???*|
|[U2800](https://www.unicode.org/charts/PDF/U2800.pdf)|Braille Patterns|*20210814-124438-54e29167*|
|[Powerline](https://github.com/ryanoasis/powerline-extra-symbols#glyphs)|Powerline triangle, curve and diagonal glyphs|*20210814-124438-54e29167*|

You can set this to `false` to use the block characters provided by your font selection.

See also [anti_alias_custom_block_glyphs](anti_alias_custom_block_glyphs.md).
