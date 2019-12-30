### Font Related Configuration

By default, wezterm will use an appropriate system-specific method for
locating the fonts that you specify using the options below.  In addition,
if you configure the `font_dirs` option, wezterm will load fonts from that
set of directories:

```toml
# This tells wezterm to look first for fonts in the directory named
# `fonts` that is found alongside your `wezterm.toml` file.
# As this option is an array, you may list multiple locations if
# you wish.
font_dirs = ["fonts"]
```

The following options impact how text is rendered:

```toml
# The font size, measured in points
font_size = 11

# The DPI to assume, measured in dots-per-inch
# This is not automatically probed!  If you experience blurry text
# or notice slight differences when comparing with other terminal
# emulators, you may wish to tune this value!
dpi = 96
```

The baseline font is configured via the `[[font.font]]` section:

```toml
[[font.font]]
# The font family name.  The default is "Menlo" on macOS,
# "Consolas" on Windows and "monospace" on X11 based systems.
# "Fira Code" to enjoy ligatures without buying an expensive font!
family = "Operator Mono SSm Lig Medium"
# Whether the font should be a bold variant
# bold = false
# Whether the font should be an italic variant
# italic = false
```

You may specify rules that apply different font styling based on
the attributes of the text rendered in the terminal.  Rules are
applied in the order that they are specified in the configuration
file, stopping with the first matching rule.

```
# Define a rule that matches when italic text is shown
[[font_rules]]
# If specified, this rule matches when a cell's italic value exactly
# matches this.  If unspecified, the attribute value is irrelevant
# with respect to matching.
italic = true

# Match based on intensity: "Bold", "Normal" and "Half" are supported
# intensity = "Normal"

# Match based on underline: "None", "Single", and "Double" are supported
# underline = "None"

# Match based on the blink attribute: "None", "Slow", "Rapid"
# blink = "None"

# Match based on reverse video
# reverse = false

# Match based on strikethrough
# strikethrough = false

# Match based on the invisible attribute
# invisible = false

  # When the above attributes match, apply this font styling
  [font_rules.font]
  font = [{family = "Operator Mono SSm Lig Medium", italic=true}]

```

Here's an example from my configuration file:

```
# Select a fancy italic font for italic text
[[font_rules]]
italic = true
  [font_rules.font]
  font = [{family = "Operator Mono SSm Lig Medium", italic=true}]

# Similarly, a fancy bold+italic font
[[font_rules]]
italic = true
intensity = "Bold"
  [font_rules.font]
  font = [{family = "Operator Mono SSm Lig", italic=true, bold=true}]

# Make regular bold text a different color to make it stand out even more
[[font_rules]]
intensity = "Bold"
  [font_rules.font]
  font = [{family = "Operator Mono SSm", bold=true}]
  foreground="tomato"

# For half-intensity text, use a lighter weight font
[[font_rules]]
intensity = "Half"
  [font_rules.font]
  font=[{family = "Operator Mono SSm Lig Light" }]
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
is approximatley equivalent to ClearType rendering on Windows, but some
people find that it appears blurry.  You may wish to try `Greyscale` in
that case.

```
font_antialias = "Subpixel" # None, Greyscale, Subpixel
font_hinting = "Full" # None, Vertical, VerticalSubpixel, Full
```


