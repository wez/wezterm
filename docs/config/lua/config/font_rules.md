# `font_rules`

You may optionally specify rules that apply different font styling based on the
attributes of the text rendered in the terminal.  Most users won't need to do
this; these rules are useful when you have some unusual fonts or mixtures of
fonts that you'd like to use.  The default is to auto-generate reasonable
italic and bold styling based on the [font](font.md) configuration.

If you do specify `font_rules`, they are applied in the order that they are
specified in the configuration file, stopping with the first matching rule:

```lua
local wezterm = require 'wezterm';
return {
  font_rules = {
    -- Define a rule that matches when italic text is shown
    {
      -- If specified, this rule matches when a cell's italic value exactly
      -- matches this.  If unspecified, the attribute value is irrelevant
      -- with respect to matching.
      italic = true,

      -- Match based on intensity: "Bold", "Normal" and "Half" are supported
      -- intensity = "Normal",

      -- Match based on underline: "None", "Single", and "Double" are supported
      -- underline = "None",

      -- Match based on the blink attribute: "None", "Slow", "Rapid"
      -- blink = "None",

      -- Match based on reverse video
      -- reverse = false,

      -- Match based on strikethrough
      -- strikethrough = false,

      -- Match based on the invisible attribute
      -- invisible = false,

      -- When the above attributes match, apply this font styling
      font = wezterm.font("Operator Mono SSm Lig Medium", {italic=true}),
    }
  }
}
```

Here's an example from my configuration file;

```lua
local wezterm = require 'wezterm';

-- A helper function for my fallback fonts
function font_with_fallback(name, params)
  local names = {name, "Noto Color Emoji", "JetBrains Mono"}
  return wezterm.font_with_fallback(names, params)
end

return {
  font = font_with_fallback("Operator Mono SSm Lig Medium"),
  font_rules= {
    -- Select a fancy italic font for italic text
    {
      italic = true,
      font = font_with_fallback("Operator Mono SSm Lig Medium Italic"),
    },

    -- Similarly, a fancy bold+italic font
    {
      italic = true,
      intensity = "Bold",
      font = font_with_fallback("Operator Mono SSm Lig Book Italic"),
    },

    -- Make regular bold text a different color to make it stand out even more
    {
      intensity = "Bold",
      font = font_with_fallback("Operator Mono SSm Lig Bold", {foreground = "tomato"}),
    },

    -- For half-intensity text, use a lighter weight font
    {
      intensity = "Half",
      font = font_with_fallback("Operator Mono SSm Lig Light"),
    },
  },
}
```
