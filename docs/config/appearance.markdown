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

  <video width="80%" controls src="../screenshots/wezterm-dynamic-colors.mp4" loop></video>

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

