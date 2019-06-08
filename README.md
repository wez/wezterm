# Wez's Terminal

A terminal emulator implemented in Rust, using OpenGL for rendering.

[![Build Status](https://travis-ci.org/wez/wezterm.svg?branch=master)](https://travis-ci.org/wez/wezterm)
[![Build status](https://ci.appveyor.com/api/projects/status/4ys3pb1vb1ja8b7h/branch/master?svg=true)](https://ci.appveyor.com/project/wez/wezterm/branch/master)

![Screenshot](screenshots/one.png)

*Screenshot of wezterm on X11, running vim*

## Installing a package

* Linux, macOS and Windows packages are available from [the Releases page](https://github.com/wez/wezterm/releases)
* Bleeding edge Windows package available from [Appveyor](https://ci.appveyor.com/project/wez/wezterm/build/artifacts?branch=master)

## Installing from source

* Install `rustup` to get the `rust` compiler installed on your system.
  https://www.rust-lang.org/en-US/install.html
* Build in release mode: `cargo build --release`
* Run it via either `cargo run --release` or `target/release/wezterm`

You will need a collection of support libraries; the [`get-deps`](get-deps) script will
attempt to install them for you.  If it doesn't know about your system,
[please contribute instructions!](CONTRIBUTING.md)

```
$ curl https://sh.rustup.rs -sSf | sh -s
$ git clone --depth=1 --branch=master --recursive https://github.com/wez/wezterm.git
$ cd wezterm
$ git submodule update --init
$ sudo ./get-deps
$ cargo build --release
$ cargo run --release -- start
```

## What?

Here's what I'm shooting for:

* A terminal escape sequence parser
* A model of a terminal screen + scrollback that is OS independent
* Textual and GUI rendering of the model
* A differential protocol for the model

This would manifest as a common core that could run as both a textual
terminal multiplexer and a gui terminal emulator, where the GUI part
could automatically provide a native UI around the remotely multiplexed
terminal session.

## Status / Features - Beta Quality

*There may be bugs that cause the terminal to panic. I'd recommend using
`tmux` or `screen` to keep your session alive if you are working on something important!*

Despite the warning above, I've been using `wezterm` as my daily driver since
the middle of Feb 2018.  The following features are done:

- [x] Runs on
 * Linux under X (requires OpenGL ES 3)
 * macOS
 * Windows 10 with [ConPty](https://blogs.msdn.microsoft.com/commandline/2018/08/02/windows-command-line-introducing-the-windows-pseudo-console-conpty/)
- [x] True Color support
- [x] Ligatures, Color Emoji and font fallback
- [x] Hyperlinks per: https://gist.github.com/egmontkob/eb114294efbcd5adb1944c9f3cb5feda
- [x] Scrollback (use mouse wheel and Shift Page{Up|Down})
- [x] xterm style selection of text with mouse; paste selection via Shift-Insert (bracketed paste is supported!)
- [x] SGR style mouse reporting (works in vim and tmux)
- [x] Render underline, double-underline, italic, bold, strikethrough
- [x] Configuration file to specify fonts and colors
- [x] Multiple Windows (Hotkey: `Super-N`)
- [x] Tabs (Hotkey: `Super-T`, next/prev: `Super-[` and `Super-]`, go-to: `Super-[1-9]`)

There's a good number of terminal escape sequences that are not yet implemented
and that will get fleshed out as the applications I use uncover them, or as folks
report them here and raise the priority.  Similarly for key mappings.  Please don't
be shy about [contributing support for missing things!](CONTRIBUTING.md)

Things that I'd like to see happen and that have no immediate priority;
[contributions to get closer to these are welcomed!](CONTRIBUTING.md)

- [ ] Sixel / iTerm2 graphics protocol support
- [ ] Textual renderer.  Think `tmux` or `screen`.
- [ ] Run on Linux with Wayland (use XWayland for now; See https://github.com/tomaka/winit/issues/306 for upstream blockers)


## Configuration

`wezterm` will look for a TOML configuration file in `$HOME/.config/wezterm/wezterm.toml`,
and then in `$HOME/.wezterm.toml`.

Configuration is currently very simple and the format is considered unstable and subject
to change.  The code for configuration can be found in [`src/config.rs`](src/config.rs).

I use the following in my `~/.wezterm.toml`:

```toml
font_size = 10
font = { font = [{family = "Operator Mono SSm Lig Medium"}] }
# How many lines of scrollback to retain
scrollback_lines = 3500

[[font_rules]]
italic = true
font = { font = [{family = "Operator Mono SSm Lig Medium", italic=true}]}

[[font_rules]]
italic = true
intensity = "Bold"
font = { font = [{family = "Operator Mono SSm Lig", italic=true, bold=true}]}

[[font_rules]]
intensity = "Bold"
  [font_rules.font]
  font = [{family = "Operator Mono SSm", bold=true}]
  # if you liked xterm's `boldColor` setting, this is how you do it in wezterm,
  # but you can apply it to any set of matching attributes!
  foreground = "tomato"

[[font_rules]]
intensity = "Half"
font = { font=[{family = "Operator Mono SSm Lig Light" }]}
```

The default configuration will attempt to use whichever font is returned from
fontconfig when `monospace` is requested.

### Shortcut / Key Binding Assignments

The default key bindings are:

| Modifiers | Key | Action |
| --------- | --- | ------ |
| `SUPER`     | `v`   | `Paste`  |
| `SHIFT`     | `Insert` | `Paste` |
| `SUPER`     | `m`      | `Hide`  |
| `SUPER`     | `n`      | `SpawnWindow` |
| `ALT`       | `Enter`  | `ToggleFullScreen` |
| `SUPER`     | `-`      | `DecreaseFontSize` |
| `CTRL`      | `-`      | `DecreaseFontSize` |
| `SUPER`     | `=`      | `IncreaseFontSize` |
| `CTRL`      | `=`      | `IncreaseFontSize` |
| `SUPER`     | `0`      | `ResetFontSize` |
| `CTRL`      | `0`      | `ResetFontSize` |
| `SUPER`     | `t`      | `SpawnTab` |
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
| `SUPER\|SHIFT` | `[` | `ActivateTabRelative(-1)` |
| `SUPER\|SHIFT` | `]` | `ActivateTabRelative(1)` |

These can be overridden using the `keys` section in your `~/.wezterm.toml` config file.
For example, you can disable a default assignment like this:

```toml
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
| `SpawnTab`         | Create a new tab in the current window |
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
```

### Colors

You can configure colors with a section like this.  In addition to specifying
SVG/CSS3 color names, you can use `#RRGGBB` to specify a color code using the
usual hex notation; eg: `#000000` is equivalent to `black`:

```toml
[colors]
foreground = "silver"
background = "black"
cursor_bg = "springgreen"
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
  your own scripts to alter the terminal appearance programmatically.

## Performance

While ultimate speed is not the main goal, performance is important!
Using the GPU to render the terminal contents helps keep CPU usage down
and the output feeling snappy.

If you want the absolute fastest terminal emulator, [alacritty](https://github.com/jwilm/alacritty)
is currently king of the crop.

## Getting help

This is a spare time project, so please bear with me.  There are two channels for support:

* You can use the GitHub issue tracker to see if someone else has a similar issue, or to file a new one: https://github.com/wez/wezterm/issues
* There is a gitter room for (potentially!) real time discussions: https://gitter.im/wezterm/Lobby

The gitter room is probably better suited to questions than it is to bug reports, but don't be afraid to use whichever you are most comfortable using and we'll work it out.


