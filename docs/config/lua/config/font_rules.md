---
tags:
  - font
---
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
config.font = wezterm.font_with_fallback 'Operator Mono SSm Lig Medium'
config.font_rules = {
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
}
```

Here's another example combining `FiraCode` with `Victor Mono`, using `Victor Mono` only for italics:

```lua
config.font = wezterm.font { family = 'FiraCode' }

config.font_rules = {
  {
    intensity = 'Bold',
    italic = true,
    font = wezterm.font {
      family = 'VictorMono',
      weight = 'Bold',
      style = 'Italic',
    },
  },
  {
    italic = true,
    intensity = 'Half',
    font = wezterm.font {
      family = 'VictorMono',
      weight = 'DemiBold',
      style = 'Italic',
    },
  },
  {
    italic = true,
    intensity = 'Normal',
    font = wezterm.font {
      family = 'VictorMono',
      style = 'Italic',
    },
  },
}
```

## Debugging Font Rules

You can run `wezterm ls-fonts` to summarize the font rules and the fonts that
match them:

```console
$ wezterm ls-fonts
Primary font:
wezterm.font_with_fallback({
  -- <built-in>, BuiltIn
  "JetBrains Mono",

  -- /home/wez/.fonts/NotoColorEmoji.ttf, FontConfig
  "Noto Color Emoji",
})


When Intensity=Half Italic=true:
wezterm.font_with_fallback({
  -- <built-in>, BuiltIn
  {family="JetBrains Mono", weight="ExtraLight", italic=true},

  -- /home/wez/.fonts/NotoColorEmoji.ttf, FontConfig
  "Noto Color Emoji",

  -- <built-in>, BuiltIn
  "JetBrains Mono",
})


When Intensity=Half Italic=false:
wezterm.font_with_fallback({
  -- <built-in>, BuiltIn
  {family="JetBrains Mono", weight="ExtraLight"},

  -- /home/wez/.fonts/NotoColorEmoji.ttf, FontConfig
  "Noto Color Emoji",

  -- <built-in>, BuiltIn
  "JetBrains Mono",
})


When Intensity=Bold Italic=false:
wezterm.font_with_fallback({
  -- <built-in>, BuiltIn
  {family="JetBrains Mono", weight="Bold"},

  -- /home/wez/.fonts/NotoColorEmoji.ttf, FontConfig
  "Noto Color Emoji",

  -- <built-in>, BuiltIn
  "JetBrains Mono",
})


When Intensity=Bold Italic=true:
wezterm.font_with_fallback({
  -- <built-in>, BuiltIn
  {family="JetBrains Mono", weight="Bold", italic=true},

  -- /home/wez/.fonts/NotoColorEmoji.ttf, FontConfig
  "Noto Color Emoji",

  -- <built-in>, BuiltIn
  "JetBrains Mono",
})


When Intensity=Normal Italic=true:
wezterm.font_with_fallback({
  -- <built-in>, BuiltIn
  {family="JetBrains Mono", italic=true},

  -- /home/wez/.fonts/NotoColorEmoji.ttf, FontConfig
  "Noto Color Emoji",

  -- <built-in>, BuiltIn
  "JetBrains Mono",
})
```

