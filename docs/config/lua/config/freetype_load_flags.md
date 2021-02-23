# `freetype_load_flags = "DEFAULT"`

*Since: nightly*

An advanced option to fine tune the freetype rasterizer.  This is a bitfield,
so you can combine one or more of these options together, separated by the `|`
character, although not many of the available options necessarily make sense to
be combined.

Available flags are:

* `DEFAULT` - This is the default!
* `NO_HINTING` - Disable hinting. This generally generates ‘blurrier’
  bitmap glyph when the glyph is rendered in any of the
  anti-aliased modes.
* `NO_BITMAP` - don't load any pre-rendered bitmap strikes
* `FORCE_AUTOHINT` - Use the freetype auto-hinter rather than the font's
  native hinter.
* `MONOCHROME` - instructs renderer to use 1-bit monochrome rendering.
  This option doesn't impact the hinter.
* `NO_AUTOHINT` - don't use the freetype auto-hinter

```lua
return {
  -- You probably don't want to do this, but this demonstrates
  -- that the flags can be combined
  freetype_load_flags = "NO_HINTING|MONOCHROME"
}
```

