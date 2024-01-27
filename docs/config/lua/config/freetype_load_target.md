---
tags:
  - font
---
# `freetype_load_target = "Normal"`

{{since('20210314-114017-04b7cedd')}}

Configures the hinting and (potentially) the rendering mode used with the
freetype rasterizer.  The following values are possible:

* `"Normal"` - This corresponds to the default hinting algorithm, optimized for standard gray-level rendering.  This is the default setting.
* `"Light"` -  A lighter hinting algorithm for non-monochrome modes. Many
  generated glyphs are more fuzzy but better resemble its
  original shape. A bit like rendering on Mac OS X.
* `"Mono"` - Strong hinting algorithm that should only be used for
  monochrome output. The result is probably unpleasant if the
  glyph is rendered in non-monochrome modes.
* `"HorizontalLcd"` - A subpixel-rendering variant of `Normal` optimized for horizontally decimated LCD displays.

See also [freetype_render_target](freetype_render_target.md) and
[freetype_load_flags](freetype_load_flags.md) for more advanced flags that can
be primarily used to influence font hinting.

Note: when using subpixel-rendering, it comes at the cost of the ability to
explicitly set the alpha channel for the text foreground color. You will need
to choose between using the alpha channel or using subpixel-rendering, and you
must select subpixel-rendering in your main configuration in order for the
correct render mode to activate: setting it only in a
[wezterm.font](../wezterm/font.md) override is not sufficient.


{{since('20240127-113634-bbcac864')}}

It is now possible to select `"VerticalLcd"` to use a subpixel-rendering
variant of `Normal` optimized for vertically decimated LCD displays.
