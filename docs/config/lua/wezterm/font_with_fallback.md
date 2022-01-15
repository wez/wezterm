# `wezterm.font_with_fallback(families [, attributes])`

This function constructs a lua table that configures a font with fallback processing.
Glyphs are looked up in the first font in the list but if missing the next font is
checked and so on.

The first parameter is a table listing the fonts in their preferred order:

```lua
local wezterm = require 'wezterm';

return {
  font = wezterm.font_with_fallback({"JetBrains Mono", "Noto Color Emoji"}),
}
```

WezTerm implicitly adds its default fallback to the list that you specify.

The *attributes* parameter behaves the same as that of [wezterm.font](font.md)
in that it allows you to specify font weight and style attributes that you
want to match.

*Since: 20210502-130208-bff6815d*

The attributes can now be specified per fallback font using this alternative
form where the family and attributes are specified as part of the same lua table:

```lua
local wezterm = require 'wezterm';

return {
  font = wezterm.font_with_fallback({
    {family="JetBrains Mono", weight="Medium"},
    {family="Terminus", weight="Bold"},
    "Noto Color Emoji"),
}
```

*Since: 20220101-133340-7edc5b5a*

You can use the expanded form mentioned above to override freetype and harfbuzz
settings just for the specified font; this examples shows how to disable the
default ligature feature just for JetBrains Mono, but leave it on for the
other fonts in the fallback:

```lua
local wezterm = require 'wezterm';

return {
  font = wezterm.font_with_fallback({
    {
      family="JetBrains Mono",
      harfbuzz_features={"calt=0", "clig=0", "liga=0"},
    },
    {family="Terminus", weight="Bold"},
    "Noto Color Emoji"),
}
```

The following options can be specified in the same way:

* [harfbuzz_features](../../font-shaping.md)
* [freetype_load_target](../config/freetype_load_target.md)
* [freetype_render_target](../config/freetype_render_target.md)
* [freetype_load_flags](../config/freetype_load_flags.md)


