---
tags:
  - font
---
# `freetype_load_flags = "DEFAULT"`

{{since('20210314-114017-04b7cedd')}}

An advanced option to fine tune the freetype rasterizer.  This is a bitfield,
so you can combine one or more of these options together, separated by the `|`
character, although not many of the available options necessarily make sense to
be combined.

Available flags are:

* `DEFAULT` - This is the default!
* `NO_HINTING` - Disable hinting. The freetype documentation says that this
  generally generates ‘blurrier’ bitmap glyph when the glyph is rendered in any of the
  anti-aliased modes, but that was written for rasterizing direct to bitmaps.
  In the context of wezterm where we are rasterizing to a texture that is then
  sampled and applied to a framebuffer through vertices on the GPU, the hinting
  process can be counter-productive and result in unexpected visual artifacts.
* `NO_BITMAP` - don't load any pre-rendered bitmap strikes
* `FORCE_AUTOHINT` - Use the freetype auto-hinter rather than the font's
  native hinter.
* `MONOCHROME` - instructs renderer to use 1-bit monochrome rendering.
  This option doesn't impact the hinter.
* `NO_AUTOHINT` - don't use the freetype auto-hinter

```lua
-- You probably don't want to do this, but this demonstrates
-- that the flags can be combined
config.freetype_load_flags = 'NO_HINTING|MONOCHROME'
```

{{since('20240128-202157-1e552d76')}}

The default value has changed to `NO_HINTING` as that generally works
more predictably and with fewer surprising artifacts.

In earlier versions, it is recommended that you configure this
explicitly:

```lua
config.freetype_load_flags = 'NO_HINTING'
```

{{since('20240203-110809-5046fc22')}}

The default value depends on the effective dpi of the display.
If the dpi is 100 or larger, the default value is `NO_HINTING`.
Otherwise, the default value is `DEFAULT`.

