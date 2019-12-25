---
title: Configuration
---

## Configuration

`wezterm` will look for a TOML configuration file in the following locations,
stopping at the first file that it finds:

* On Windows, `wezterm.toml` from the directory that contains `wezterm.exe`.
  This is handy for users that want to carry their wezterm install around on a thumb drive.
* `$HOME/.config/wezterm/wezterm.toml`,
* `$HOME/.wezterm.toml`

`wezterm` will watch the config file that it loads; if/when it changes, the configuration
will be automatically reloaded and the majority of options will take effect immediately.

Configuration is currently very simple and the format is considered unstable and subject
to change.  The code for configuration can be found in [`src/config/mod.rs`](https://github.com/wez/wezterm/blob/master/src/config/mod.rs).

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

### Advanced Font Shaping Options

The `harfbuzz_features` option allows specifying the features to enable when
using harfbuzz for font shaping.

There is some light documentation here:
<https://harfbuzz.github.io/shaping-opentype-features.html>
but it boils down to allowing opentype feature names to be specified
using syntax similar to the CSS font-feature-settings options:
<https://developer.mozilla.org/en-US/docs/Web/CSS/font-feature-settings>.
The OpenType spec lists a number of features here:
<https://docs.microsoft.com/en-us/typography/opentype/spec/featurelist>

Options of likely interest will be:

* `calt` - <https://docs.microsoft.com/en-us/typography/opentype/spec/features_ae#tag-calt>
* `clig` - <https://docs.microsoft.com/en-us/typography/opentype/spec/features_ae#tag-clig>

If you want to disable ligatures in most fonts, then you may want to
use a setting like this:

```toml
harfbuzz_features = ["calt=0", "clig=0", "liga=0"]
```

Some fonts make available extended options via stylistic sets.
If you use the [Fira Code font](https://github.com/tonsky/FiraCode),
it lists available stylistic sets here:
<https://github.com/tonsky/FiraCode/wiki/How-to-enable-stylistic-sets>

and you can set them in wezterm:

```toml
# Use this for a zero with a dot rather than a line through it
# when using the Fira Code font
harfbuzz_features = ["zero"]
```

### Misc configuration

```toml
# How many lines of scrollback you want to retain per tab
scrollback_lines = 3500

# Enable the scrollbar.  This is currently disabled by default.
# It will occupy the right window padding space.
# If right padding is set to 0 then it will be increased
# to a single cell width
enable_scroll_bar = true

# If no `prog` is specified on the command line, use this
# instead of running the user's shell.
# The value is the argument array, with the 0th element being
# the executable to run.  The path will be searched to locate
# this if needed.
# For example, to have `wezterm` always run `top` by default,
# you'd use this:
default_prog = ["top"]

# What to set the TERM variable to
term = "xterm-256color"

# Constrains the rate at which output from a child command is
# processed and applied to the terminal model.
# This acts as a brake in the case of a command spewing a
# ton of output and allows for the UI to remain responsive
# so that you can hit CTRL-C to interrupt it if desired.
# The default value is 200,000 bytes/s.
ratelimit_output_bytes_per_second = 200_000

# Constrains the rate at which the multiplexer server will
# unilaterally push data to the client.
# This helps to avoid saturating the link between the client
# and server.
# Each time the screen is updated as a result of the child
# command outputting data (rather than in response to input
# from the client), the server considers whether to push
# the result to the client.
# That decision is throttled by this configuration value
# which has a default value of 10/s
ratelimit_mux_output_pushes_per_second = 10

# Constrain how often the mux server scans the terminal
# model to compute a diff to send to the mux client.
# The default value is 100/s
ratelimit_mux_output_scans_per_second = 100

# If false, do not try to use a Wayland protocol connection
# when starting the gui frontend, and instead use X11.
# This option is only considered on X11/Wayland systems and
# has no effect on macOS or Windows.
# The default is true.
enable_wayland = true


# Specifies how often a blinking cursor transitions between visible
# and invisible, expressed in milliseconds.
# Setting this to 0 disables blinking.
# Note that this value is approximate due to the way that the system
# event loop schedulers manage timers; non-zero values will be at
# least the interval specified with some degree of slop.
# It is recommended to avoid blinking cursors when on battery power,
# as it is relatively costly to keep re-rendering for the blink!
cursor_blink_rate = 800

# Specifies the default cursor style.  various escape sequences
# can override the default style in different situations (eg:
# an editor can change it depending on the mode), but this value
# controls how the cursor appears when it is reset to default.
# The default is `SteadyBlock`.
# Acceptable values are `SteadyBlock`, `BlinkingBlock`,
# `SteadyUnderline`, `BlinkingUnderline`, `SteadyBar`,
# and `BlinkingBar`.
default_cursor_style = "SteadyBlock"
```

### Shortcut / Key Binding Assignments

The default key bindings are:

| Modifiers | Key | Action |
| --------- | --- | ------ |
| `SUPER`     | `c`   | `Copy`  |
| `SUPER`     | `v`   | `Paste`  |
| `CTRL|SHIFT`     | `c`   | `Copy`  |
| `CTRL|SHIFT`     | `v`   | `Paste`  |
| `SHIFT`     | `Insert` | `Paste` |
| `SUPER`     | `m`      | `Hide`  |
| `SUPER`     | `n`      | `SpawnWindow` |
| `CTRL|SHIFT`     | `n`      | `SpawnWindow` |
| `ALT`       | `Enter`  | `ToggleFullScreen` |
| `SUPER`     | `-`      | `DecreaseFontSize` |
| `CTRL`      | `-`      | `DecreaseFontSize` |
| `SUPER`     | `=`      | `IncreaseFontSize` |
| `CTRL`      | `=`      | `IncreaseFontSize` |
| `SUPER`     | `0`      | `ResetFontSize` |
| `CTRL`      | `0`      | `ResetFontSize` |
| `SUPER`     | `t`      | `SpawnTabInCurrentTabDomain` |
| `CTRL|SHIFT`     | `t`      | `SpawnTabInCurrentTabDomain` |
| `SUPER|SHIFT` | `T`    | `SpawnTab` |
| `SUPER`     | `w`      | `CloseCurrentTab` |
| `SUPER`     | `1`      | `ActivateTab(0)` |
| `SUPER`     | `2`      | `ActivateTab(1)` |
| `SUPER`     | `3`      | `ActivateTab(2)` |
| `SUPER`     | `4`      | `ActivateTab(3)` |
| `SUPER`     | `5`      | `ActivateTab(4)` |
| `SUPER`     | `6`      | `ActivateTab(5)` |
| `SUPER`     | `7`      | `ActivateTab(6)` |
| `SUPER`     | `8`      | `ActivateTab(7)` |
| `SUPER`     | `9`      | `ActivateTab(8)` |
| `CTRL|SHIFT`     | `w`      | `CloseCurrentTab` |
| `CTRL|SHIFT`     | `1`      | `ActivateTab(0)` |
| `CTRL|SHIFT`     | `2`      | `ActivateTab(1)` |
| `CTRL|SHIFT`     | `3`      | `ActivateTab(2)` |
| `CTRL|SHIFT`     | `4`      | `ActivateTab(3)` |
| `CTRL|SHIFT`     | `5`      | `ActivateTab(4)` |
| `CTRL|SHIFT`     | `6`      | `ActivateTab(5)` |
| `CTRL|SHIFT`     | `7`      | `ActivateTab(6)` |
| `CTRL|SHIFT`     | `8`      | `ActivateTab(7)` |
| `CTRL|SHIFT`     | `9`      | `ActivateTab(8)` |
| `SUPER\|SHIFT` | `[` | `ActivateTabRelative(-1)` |
| `SUPER\|SHIFT` | `]` | `ActivateTabRelative(1)` |

These can be overridden using the `keys` section in your `~/.wezterm.toml` config file.
For example, you can disable a default assignment like this:

```
# Turn off the default CMD-m Hide action
[[keys]]
key = "m"
mods = "CMD"
action = "Nop"
```

The `key` value can be one of the following keycode identifiers.  Note that not
all of these are meaningful on all platforms:

`Hyper`, `Super`, `Meta`, `Cancel`, `Backspace`, `Tab`, `Clear`, `Enter`,
`Shift`, `Escape`, `LeftShift`, `RightShift`, `Control`, `LeftControl`,
`RightControl`, `Alt`, `LeftAlt`, `RightAlt`, `Menu`, `LeftMenu`, `RightMenu`,
`Pause`, `CapsLock`, `PageUp`, `PageDown`, `End`, `Home`, `LeftArrow`,
`RightArrow`, `UpArrow`, `DownArrow`, `Select`, `Print`, `Execute`,
`PrintScreen`, `Insert`, `Delete`, `Help`, `LeftWindows`, `RightWindows`,
`Applications`, `Sleep`, `Numpad0`, `Numpad1`, `Numpad2`, `Numpad3`,
`Numpad4`, `Numpad5`, `Numpad6`, `Numpad7`, `Numpad8`, `Numpad9`, `Multiply`,
`Add`, `Separator`, `Subtract`, `Decimal`, `Divide`, `NumLock`, `ScrollLock`,
`BrowserBack`, `BrowserForward`, `BrowserRefresh`, `BrowserStop`,
`BrowserSearch`, `BrowserFavorites`, `BrowserHome`, `VolumeMute`,
`VolumeDown`, `VolumeUp`, `MediaNextTrack`, `MediaPrevTrack`, `MediaStop`,
`MediaPlayPause`, `ApplicationLeftArrow`, `ApplicationRightArrow`,
`ApplicationUpArrow`, `ApplicationDownArrow`.

Alternatively, a single unicode character can be specified to indicate
pressing the corresponding key.

Possible Modifier labels are:

 * `SUPER`, `CMD`, `WIN` - these are all equivalent: on macOS the `Command` key,
   on Windows the `Windows` key, on Linux this can also be the `Super` or `Hyper`
   key.  Left and right are equivalent.
 * `SHIFT` - The shift key.  Left and right are equivalent.
 * `ALT`, `OPT`, `META` - these are all equivalent: on macOS the `Option` key,
   on other systems the `Alt` or `Meta` key.  Left and right are equivalent.

You can combine modifiers using the `|` symbol (eg: `"CMD|CTRL"`).

Possible actions are listed below.  Some actions require a parameter that is
specified via the `arg` key; see examples below.

| Name               | Effect             |
| ------------------ | ------------------ |
| `SpawnTab`         | Create a new local tab in the current window |
| `SpawnTabInCurrentTabDomain` | Create a new tab in the current window. The tab will be spawned in the same domain as the currently active tab |
| `SpawnTabInDomain` | Create a new tab in the current window. The tab will be spawned in the domain specified by the `arg` value |
| `SpawnWindow`      | Create a new window |
| `ToggleFullScreen` | Toggles full screen mode for current window |
| `Paste`            | Paste the clipboard to the current tab |
| `ActivateTabRelative` | Activate a tab relative to the current tab.  The `arg` value specifies an offset. eg: `-1` activates the tab to the left of the current tab, while `1` activates the tab to the right. |
| `ActivateTab` | Activate the tab specified by the `arg` value. eg: `0` activates the leftmost tab, while `1` activates the second tab from the left, and so on. |
| `IncreaseFontSize` | Increases the font size of the current window by 10% |
| `DecreaseFontSize` | Decreases the font size of the current window by 10% |
| `ResetFontSize` | Reset the font size for the current window to the value in your configuration |
| `SendString` | Sends the string specified by the `arg` value to the terminal in the current tab, as though that text were literally typed into the terminal. |
| `Nop` | Does nothing.  This is useful to disable a default key assignment. |
| `Hide` | Hides the current window |
| `Show` | Shows the current window |
| `CloseCurrentTab` | Equivalent to clicking the `x` on the window title bar to close it: Closes the current tab.  If that was the last tab, closes that window.  If that was the last window, wezterm terminates. |

Example:

```toml
# Turn off the default CMD-m Hide action
[[keys]]
key = "m"
mods = "CMD"
action = "Nop"

# Macro for sending in some boiler plate.  This types `wtf!?` each
# time CMD+SHIFT+W is pressed
[[keys]]
key = "W"
mods = "CMD|SHIFT"
action = "SendString"
arg = "wtf!?"

# CTRL+ALT+0 activates the leftmost tab
[[keys]]
key = "0"
mods = "CTRL|ALT"
action = "ActivateTab"
# the tab number
arg = "0"

# CMD+y spawns a new tab in Domain 1
[[keys]]
key = "y"
mods = "CMD"
action = "SpawnTabInDomain"
# the domain ID
arg = "1"
```

### Window Padding

You may add padding around the edges of the terminal cells:

```
[window_padding]
left = 2

# This will become the scrollbar width if you have enabled the scrollbar!
right = 2

top = 0
bottom = 0
```

### Colors

You can configure colors with a section like this.  In addition to specifying
[SVG/CSS3 color names](https://docs.rs/palette/0.4.1/palette/named/index.html#constants),
you can use `#RRGGBB` to specify a color code using the
usual hex notation; eg: `#000000` is equivalent to `black`:

```toml
[colors]
# The default text color
foreground = "silver"
# The default background color
background = "black"

# Overrides the cell background color when the current cell is occupied by the
# cursor and the cursor style is set to Block
cursor_bg = "#52ad70"
# Overrides the text color when the current cell is occupied by the cursor
cursor_fg = "black"
# Specifies the border color of the cursor when the cursor style is set to Block,
# of the color of the vertical or horizontal bar when the cursor style is set to
# Bar or Underline.
cursor_border = "#52ad70"

# The color of the scrollbar "thumb"; the portion that represents the current viewport
scrollbar_thumb = "#222222"

ansi = ["black", "maroon", "green", "olive", "navy", "purple", "teal", "silver"]
brights = ["grey", "red", "lime", "yellow", "blue", "fuchsia", "aqua", "white"]
```

You can find a variety of color schemes [here](https://github.com/mbadolato/iTerm2-Color-Schemes).
There are two ways to use them with wezterm:

* [The wezterm directory](https://github.com/mbadolato/iTerm2-Color-Schemes/tree/master/wezterm) contains
  configuration snippets that you can copy and paste into your `wezterm.toml` file
  to set the default configuration.
* [The dynamic-colors directory](https://github.com/mbadolato/iTerm2-Color-Schemes/tree/master/dynamic-colors)
  contains shell scripts that can change the color scheme immediately on the fly.
  This is super convenient for trying out color schemes, and can be used in
  your own scripts to alter the terminal appearance programmatically:

```bash
$ git clone https://github.com/mbadolato/iTerm2-Color-Schemes.git
$ cd iTerm2-Color-Schemes/dynamic-colors
$ for scheme in *.sh ; do ; echo $scheme ; \
   bash "$scheme" ; ../tools/screenshotTable.sh; sleep 0.5; done
```

  <video width="80%" controls src="screenshots/wezterm-dynamic-colors.mp4" loop></video>

### Tab Bar Colors

The following options control the appearance of the tab bar:

```toml
[colors.tab_bar]
# The color of the strip that goes along the top of the window
background = "#0b0022"

# The active tab is the one that has focus in the window
[colors.tab_bar.active_tab]
# The color of the background area for the tab
bg_color = "#2b2042"
# The color of the text for the tab
fg_color = "#c0c0c0"

# Specify whether you want "Half", "Normal" or "Bold" intensity for the
# label shown for this tab.
# The default is "Normal"
intensity = "Normal"

# Specify whether you want "None", "Single" or "Double" underline for
# label shown for this tab.
# The default is "None"
underline = "None"

# Specify whether you want the text to be italic (true) or not (false)
# for this tab.  The default is false.
italic = false

# Specify whether you want the text to be rendered with strikethrough (true)
# or not for this tab.  The default is false.
strikethrough = false

# Inactive tabs are the tabs that do not have focus
[colors.tab_bar.inactive_tab]
bg_color = "#1b1032"
fg_color = "#808080"

# The same options that were listed under the `active_tab` section above
# can also be used for `inactive_tab`.

# You can configure some alternate styling when the mouse pointer
# moves over inactive tabs
[colors.tab_bar.inactive_tab_hover]
bg_color = "#3b3052"
fg_color = "#909090"
italic = true

# The same options that were listed under the `active_tab` section above
# can also be used for `inactive_tab_hover`.
```

