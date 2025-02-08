---
search:
  boost: 10
keywords: font
tags:
 - font
title: wezterm.font
---

# `wezterm.font(family [, attributes])`

This function constructs a lua table that corresponds to the internal `FontAttributes`
struct that is used to select a single named font:

```lua
local wezterm = require 'wezterm'

return {
  font = wezterm.font 'JetBrains Mono',
}
```

The first parameter is the name of the font; the name can be one of the following types of names:

* The font family name, eg: `"JetBrains Mono"`.  The family name doesn't include any style information (such as weight, stretch or italic), which can be specified via the *attributes* parameter.  **This is the recommended name to use for the font**, as it the most compatible way to resolve an installed font.
* The computed *full name*, which is the family name with the sub-family (which incorporates style information) appended, eg: `"JetBrains Mono Regular"`.
* (Since 20210502-154244-3f7122cb) The *postscript name*, which is an ostensibly unique name identifying a given font and style that is encoded into the font by the font designer.

When specifying a font using its family name, the second *attributes* parameter
is an optional table that can be used to specify style attributes; the
following keys are allowed:

* `weight` - specifies the weight of the font.  The default value is `"Regular"`, and possible values are:

    * `"Thin"`
    * `"ExtraLight"`
    * `"Light"`
    * `"DemiLight"`
    * `"Book"`
    * `"Regular"` (this is the default)
    * `"Medium"`
    * `"DemiBold"`
    * `"Bold"`
    * `"ExtraBold"`
    * `"Black"`
    * `"ExtraBlack"`.

  `weight` has been supported since version 20210502-130208-bff6815d, In earlier versions you
  could use `bold=true` to get a bold font variant.

* `stretch` - specifies the font stretch to select.  The default value is `"Normal"`, and possible values are:

    * `"UltraCondensed"`
    * `"ExtraCondensed"`
    * `"Condensed"`
    * `"SemiCondensed"`
    * `"Normal"` (this is the default)
    * `"SemiExpanded"`
    * `"Expanded"`
    * `"ExtraExpanded"`
    * `"UltraExpanded"`.

  `stretch` has been supported since version 20210502-130208-bff6815d.

* `style` - specifies the font style to select.  The default is `"Normal"`, and possible values are:

    * `"Normal"` (this is the default)
    * `"Italic"`
    * `"Oblique"`

  `"Oblique"` and `"Italic"` fonts are similar in the sense that the glyphs
  are presented at an angle.  `"Italic"` fonts usually have a distinctive
  design difference from the `"Normal"` style in a given font family,
  whereas `"Oblique"` usually looks very similar to `"Normal"`, but skewed
  at an angle.

  `style` has been supported since version 20220319-142410-0fcdea07. In earlier versions
  you could use `italic=true` to get an italic font variant.

When attributes are specified, the font must match both the family name and
attributes in order to be selected.

With the exception of being able to synthesize basic bold and italics (really,
oblique) for non-bitmap fonts, wezterm can only select and use fonts that you
have installed on your system.  The attributes that you specify are used to
match a font from those that are available, so if you'd like to use a condensed
font, for example, then you must install the condensed variant of that family.


```lua
local wezterm = require 'wezterm'

return {
  font = wezterm.font('JetBrains Mono', { weight = 'Bold' }),
}
```

When resolving fonts from [font_dirs](../config/font_dirs.md), wezterm follows CSS Fonts
Level 3 compatible font matching, which tries to exactly match the specified
attributes, but allows for locating a close match within the specified font
family.

```lua
local wezterm = require 'wezterm'

return {
  font = wezterm.font(
    'Iosevka Term',
    { stretch = 'Expanded', weight = 'Regular' }
  ),
}
```

An alternative form of specifying the font can be used, where the family and the attributes
are combined in the same lua table.  This form is most useful when used together with
[wezterm.font_with_fallback](font_with_fallback.md) when you want to specify precise
weights for the different fallback fonts:

```lua
local wezterm = require 'wezterm'

return {
  font = wezterm.font {
    family = 'Iosevka Term',
    stretch = 'Expanded',
    weight = 'Regular',
  },
}
```

{{since('20220101-133340-7edc5b5a')}}

You can use the expanded form mentioned above to override freetype and harfbuzz
settings just for the specified font; this examples shows how to disable the
default ligature feature just for this particular font:

```lua
local wezterm = require 'wezterm'
return {
  font = wezterm.font {
    family = 'JetBrains Mono',
    harfbuzz_features = { 'calt=0', 'clig=0', 'liga=0' },
  },
}
```

The following options can be specified in the same way:

* [harfbuzz_features](../../font-shaping.md)
* [freetype_load_target](../config/freetype_load_target.md)
* [freetype_render_target](../config/freetype_render_target.md)
* [freetype_load_flags](../config/freetype_load_flags.md)
* `assume_emoji_presentation = true` or `assume_emoji_presentation = false` to control whether a font is considered to have emoji (rather than text) presentation glyphs for emoji. {{since('20220807-113146-c2fee766', inline=True)}}

