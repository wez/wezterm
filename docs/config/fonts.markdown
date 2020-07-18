### Font Related Configuration

By default, wezterm will use an appropriate system-specific method for
locating the fonts that you specify using the options below.  In addition,
if you configure the `font_dirs` option, wezterm will load fonts from that
set of directories:

```lua
return {
  -- This tells wezterm to look first for fonts in the directory named
  -- `fonts` that is found alongside your `wezterm.toml` file.
  -- As this option is an array, you may list multiple locations if
  -- you wish.
  font_dirs = {"fonts"},
}
```

The following options impact how text is rendered:

```lua
return {
  -- The font size, measured in points
  font_size = 11.0,

  -- The DPI to assume, measured in dots-per-inch
  -- This is not automatically probed!  If you experience blurry text
  -- or notice slight differences when comparing with other terminal
  -- emulators, you may wish to tune this value!
  dpi = 96.0,

  -- When true (the default), text that is set to ANSI color
  -- indices 0-7 will be shifted to the corresponding brighter
  -- color index (8-15) when the intensity is set to Bold.
  --
  -- This brightening effect doesn't occur when the text is set
  -- to the default foreground color!
  --
  -- This defaults to true for better compatibility with a wide
  -- range of mature software; for instance, a lot of software
  -- assumes that Black+Bold renders as a Dark Grey which is
  -- legible on a Black background, but if this option is set to
  -- true, it would render as Black on Black.
  bold_brightens_ansi_colors = true,
}
```

To select the font face you can use a helper function imported
from the `wezterm` module:

```lua
local wezterm = require 'wezterm';

return {
  -- The font family name.  The default is "Menlo" on macOS,
  -- "Consolas" on Windows and "monospace" on X11 based systems.
  -- You may wish to download and try either "JetBrains Mono" or
  -- "Fira Code" to enjoy ligatures without buying an expensive font!
  font = wezterm.font("Operator Mono SSm Lig Medium"),

  -- You can specify some parameters to influence the font selection;
  -- for example, this selects a Bold, Italic font variant.
  font = wezterm.font("JetBrains Mono", {bold=true, italic=true})
}
```

If you'd like to specify fallback fonts (eg: you've got a killer
monospace font, but it doesn't have glyphs for the asian script
that you sometimes work with), you can specify multiple fonts that
wezterm will use in the order you specify:

```lua
local wezterm = require 'wezterm';
return {
  font = wezterm.font_with_fallback({
    "My Preferred Font",
    -- This is searched for glyphs that aren't found in the first one
    "My second best font",
    -- This is searched for glyphs that aren't found in either of
    -- the first two
    "My third best font",
  }),
}
```

You may optionally specify rules that apply different font styling based on the
attributes of the text rendered in the terminal.  Most users won't need to do
this; these rules are useful when you have some unusual fonts or mixtures of
fonts that you'd like to use.  The default is to auto-generate reasonable
italic and bold styling based on the `font` configuration.

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

There are a couple of additional advanced font configuration options:

* `font_locator` - specifies the method by which system fonts are
  located and loaded.  You may specify `ConfigDirsOnly` to disable
  loading system fonts and use only the fonts found in the directories
  that you specify in your `font_dirs` configuration option.  Otherwise,
  it is recommended to omit this setting.
* `font_shaper` - specifies the method by which text is mapped to glyphs
  in the available fonts.  The shaper is responsible for handling
  kerning, ligatures and emoji composition.  The default is `Harfbuzz`
  and we have very preliminary support for `Allsorts`.
* `font_rasterizer` - specifies the method by which fonts are rendered
  on screen.  The only available implementation is `FreeType`.

These options affect the appearance of the text.  `Subpixel` antialiasing
is approximateley equivalent to ClearType rendering on Windows, but some
people find that it appears blurry.  You may wish to try `Greyscale` in
that case.

```lua
return {
  font_antialias = "Subpixel", -- None, Greyscale, Subpixel
  font_hinting = "Full",  -- None, Vertical, VerticalSubpixel, Full
}
```

