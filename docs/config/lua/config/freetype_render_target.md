---
tags:
  - font
---
# `freetype_render_target = "Normal"`

{{since('20210502-130208-bff6815d')}}

Configures the *rendering* mode used with the freetype rasterizer.

The default is to use the value of [freetype_load_target](freetype_load_target.md).

You may wish to override that value if you want very fine control over
how freetype hints and then renders glyphs.

For example, this configuration uses light hinting but produces
subpixel-antialiased glyph bitmaps:

```lua
config.freetype_load_target = 'Light'
config.freetype_render_target = 'HorizontalLcd'
```

