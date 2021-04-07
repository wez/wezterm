# `font_shaper`

specifies the method by which text is mapped to glyphs in the available fonts.
The shaper is responsible for handling kerning, ligatures and emoji
composition.  The default is `Harfbuzz` and we have very preliminary support
for `Allsorts`.

It is strongly recommended that you use the default `Harfbuzz` shaper.

