# `font_rules`

When textual output in the terminal is styled with bold, italic or other
attributes, wezterm uses `font_rules` to decide how to render that text.

By default, unstyled text will use the font specified by the [font](font.md)
configuration, and wezterm will use that as a base, and then automatically
generate appropriate `font_rules` that use heavier weight fonts for bold text,
lighter weight fonts for dim text and italic fonts for italic text.

Most users won't need to specify any explicit value for `font_rules`, as the
defaults should be sufficient.

If you have some unusual fonts or mixtures of fonts that you'd like to use,
such as using your favourite monospace font for the base and a fancy italic
font from a different font family for italics, then `font_rules` will allow you
to do so.

`font_rules` is comprised of a list of rule entries with fields that are split
into *matcher* fields and *action* fields. Matcher fields specify which textual
attributes you want to match on, while action fields specify how you want to
render them.

The following fields are matcher fields:

|Name      |Associated Attribute|Possible Values|
|----------|--------------------|---------------|
|italic    |italic              |`true` (italic) or `false` (not italic)|
|intensity |bold/bright or dim/half-bright|`"Normal"` (neither bold nor dim), `"Bold"`, `"Half"`|
|underline |underline           | `"None"` (not underlined), `"Single"` (single underline), `"Double"` (double underline)|
|blink     |blinking            | `"None"` (not blinking), `"Rapid"` (regular rapid blinking), `"Slow"` (slow blinking)|
|reverse   |reverse/inverse     | `true` (reversed) or `false` (not reversed)|
|strikethrough|strikethrough    | `true` (struck-through) or `false` (not struck-through)|
|invisible |invisible           | `true` (invisible) or `false` (not invisible)|

If a matcher field is omitted, then the associated attribute has no impact
on the match: the rule *doesn't care about* that attribute and will match based
on the attributes that were listed.

The following fields are action fields:

|Name          |Action   |
|--------------|---------|
|font          |Specify the font that should be used|

The way that `font_rules` are processed is:

1. Take the list of `font_rules` from your configuration
2. For each rule in the order listed in `font_rules`:
3. Consider each *matcher* field explicitly specified in the entry. If the associated attribute does not match the value specified in the entry, move on to the next rule.
4. If all of the *matcher* fields explicitly specified in the entry match the attributes of the text, then:
   * The `font` *action* field, if specified, will override the base `font` configuration
   * No further `font_rules` will be considered: the matching is complete
5. If none of the rules you specify matched, then a set of default rules based on your base `font` will be used in the same way as above.

Here's an example from my configuration file, which I use with a variant of
`Operator Mono` that is patched to add ligatures.  This particular font has
font-weights that are either too bold or too light for the default rules to
produce great results, hence this set of rules.

```lua
local wezterm = require 'wezterm'

return {
  font = wezterm.font_with_fallback 'Operator Mono SSm Lig Medium',
  font_rules = {
    -- For Bold-but-not-italic text, use this relatively bold font, and override
    -- its color to a tomato-red color to make bold text really stand out.
    {
      intensity = 'Bold',
      italic = false,
      font = wezterm.font_with_fallback(
        'Operator Mono SSm Lig',
        -- Override the color specified by the terminal output and force
        -- it to be tomato-red.
        -- The color value you set here can be any CSS color name or
        -- RGB color string.
        { foreground = 'tomato' }
      ),
    },

    -- Bold-and-italic
    {
      intensity = 'Bold',
      italic = true,
      font = wezterm.font_with_fallback {
        family = 'Operator Mono SSm Lig',
        italic = true,
      },
    },

    -- normal-intensity-and-italic
    {
      intensity = 'Normal',
      italic = true,
      font = wezterm.font_with_fallback {
        family = 'Operator Mono SSm Lig',
        weight = 'DemiLight',
        italic = true,
      },
    },

    -- half-intensity-and-italic (half-bright or dim); use a lighter weight font
    {
      intensity = 'Half',
      italic = true,
      font = wezterm.font_with_fallback {
        family = 'Operator Mono SSm Lig',
        weight = 'Light',
        italic = true,
      },
    },

    -- half-intensity-and-not-italic
    {
      intensity = 'Half',
      italic = false,
      font = wezterm.font_with_fallback {
        family = 'Operator Mono SSm Lig',
        weight = 'Light',
      },
    },
  },
}
```

Here's another example combining `FiraCode` with `Victor Mono`, using `Victor Mono` only for italics:

```lua
local wezterm = require 'wezterm'
return {
  color_scheme = 'Zenburn',
  font_size = 8.0,
  harfbuzz_features = { 'kern', 'liga', 'clig', 'calt' },
  font = wezterm.font_with_fallback {
    {
      family = 'FiraCode',
      weight = 'Regular',
      -- see: https://github.com/tonsky/FiraCode/wiki/How-to-enable-stylistic-sets
      harfbuzz_features = { 'cv13', 'cv18', 'cv31', 'ss03' },
    },
  },
  font_rules = {
    -- Select a fancy italic font for italic text
    {
      italic = true,
      intensity = 'Normal',
      font = wezterm.font {
        family = 'VictorMono',
        weight = 'Medium',
        style = 'Italic',
      },
    },
    -- Similarly, a fancy bold+italic font
    {
      intensity = 'Bold',
      italic = true,
      font = wezterm.font {
        family = 'VictorMono',
        weight = 'Bold',
        style = 'Italic',
      },
    },
    -- Regular bold
    {
      intensity = 'Bold',
      font = wezterm.font {
        family = 'FiraCode',
        weight = 'DemiBold',
        style = 'Normal',
        harfbuzz_features = { 'cv13', 'cv18', 'cv31', 'ss03' },
      },
    },
    -- For half-intensity italic
    {
      italic = true,
      intensity = 'Half',
      font = wezterm.font {
        family = 'VictorMono',
        weight = 'Light',
        style = 'Italic',
      },
    },
    -- Make half-intensity have same zero as FiraCode
    {
      intensity = 'Half',
      italic = false,
      font = wezterm.font {
        family = 'VictorMono',
        weight = 'Light',
        -- ss04 change zero, affects poundsign for Fira
        -- https://github.com/rubjo/victor-mono
        harfbuzz_features = { 'ss04' },
      },
    },
  },
}
```

## Debugging Font Rules

You can run `wezterm ls-fonts` to summarize the font rules and the fonts that
match them.
