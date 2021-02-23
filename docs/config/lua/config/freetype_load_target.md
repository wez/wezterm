# `freetype_load_target = "Normal"`

*Since: nightly*

Configures the rendering mode used with the freetype rasterizer.
The following values are possible:

* `"Normal"` - This corresponds to the default hinting algorithm, optimized for standard gray-level rendering.  This is the default setting.
* `"Light"` -  A lighter hinting algorithm for non-monochrome modes. Many
  generated glyphs are more fuzzy but better resemble its
  original shape. A bit like rendering on Mac OS X.
* `"Mono"` - Strong hinting algorithm that should only be used for
  monochrome output. The result is probably unpleasant if the
  glyph is rendered in non-monochrome modes.
* `"HorizontalLcd"` - A subpixel-rendering variant of `Normal` optimized for horizontally decimated LCD displays.

See also [freetype_load_flags](freetype_load_flags.md) for more advanced flags
that can be primarily used to influence font hinting.

