## `custom_block_glyphs = true`

*Since: nightly builds only*

When set to `true` (the default), WezTerm will compute its own idea of what the
[U2580](https://www.unicode.org/charts/PDF/U2580.pdf) unicode block elements
range, instead of using glyphs resolved from a font.

Ideally this option wouldn't exist, but it is present to work around a [hinting issue in freetype](https://gitlab.freedesktop.org/freetype/freetype/-/issues/761).

You can set this to `false` to use the block characters provided by your font selection.


