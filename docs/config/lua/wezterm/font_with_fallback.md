---
title: wezterm.font_with_fallback
tags:
 - font
---

# `wezterm.font_with_fallback(families [, attributes])`

This function constructs a lua table that configures a font with fallback processing.
Glyphs are looked up in the first font in the list but if missing the next font is
checked and so on.

The first parameter is a table listing the fonts in their preferred order:

```lua
local wezterm = require 'wezterm'

return {
  font = wezterm.font_with_fallback { 'JetBrains Mono', 'Noto Color Emoji' },
}
```

WezTerm implicitly adds its default fallback to the list that you specify.

The *attributes* parameter behaves the same as that of [wezterm.font](font.md)
in that it allows you to specify font weight and style attributes that you
want to match.

{{since('20210502-130208-bff6815d')}}

The attributes can now be specified per fallback font using this alternative
form where the family and attributes are specified as part of the same lua table:

```lua
local wezterm = require 'wezterm'

return {
  font = wezterm.font_with_fallback {
    { family = 'JetBrains Mono', weight = 'Medium' },
    { family = 'Terminus', weight = 'Bold' },
    'Noto Color Emoji',
  },
}
```

{{since('20220101-133340-7edc5b5a')}}

You can use the expanded form mentioned above to override freetype and harfbuzz
settings just for the specified font; this examples shows how to disable the
default ligature feature just for JetBrains Mono, but leave it on for the
other fonts in the fallback:

```lua
local wezterm = require 'wezterm'

return {
  font = wezterm.font_with_fallback {
    {
      family = 'JetBrains Mono',
      harfbuzz_features = { 'calt=0', 'clig=0', 'liga=0' },
    },
    { family = 'Terminus', weight = 'Bold' },
    'Noto Color Emoji',
  },
}
```

The following options can be specified in the same way:

* [harfbuzz_features](../../font-shaping.md)
* [freetype_load_target](../config/freetype_load_target.md)
* [freetype_render_target](../config/freetype_render_target.md)
* [freetype_load_flags](../config/freetype_load_flags.md)
* `assume_emoji_presentation = true` or `assume_emoji_presentation = false` to control whether a font is considered to have emoji (rather than text) presentation glyphs for emoji. {{since('20220807-113146-c2fee766', inline=True)}}

## Dealing with different fallback font heights

When mixing different font families there is a chance that glyphs from one font
don't appear to be the same height as the glyphs from your primary font.

For "Roman" fonts there exists a font metric known as *cap-height* which
indicates the nominal size of a capital (uppercase) letter that can be used to
compute a scaling factor that can be used to make the fallback font appear to
have the same size.

Setting
[use_cap_height_to_scale_fallback_fonts](../config/use_cap_height_to_scale_fallback_fonts.md)
= `true` will cause wezterm to try to automatically scale using the
*cap-height* metric (or to compute its own idea of the *cap-height* based on the size of
glyph(s)).

### Manual fallback scaling

{{since('20220408-101518-b908e2dd')}}

CJK fonts typically won't have a useful *cap-height* metric so it may be
desirable to manually configure the fallback scaling factor to boost the size
of the CJK font so that the glyphs are more readable.

The example below shows how to boost the effective size of the `"Microsoft
YaHei"` fallback font to `1.5` times the normal size.  The boost cannot
influence font metrics so it may be desirable to also specify
[line_height](../config/line_height.md) to produce a more pleasing display.

```lua
local wezterm = require 'wezterm'

return {
  line_height = 1.2,
  font = wezterm.font_with_fallback {
    'JetBrains Mono',
    { family = 'Microsoft YaHei', scale = 1.5 },
  },
}
```
