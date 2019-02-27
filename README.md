# Wez's Terminal

A terminal emulator implemented in Rust, using OpenGL for rendering.

[![Build Status](https://travis-ci.org/wez/wezterm.svg?branch=master)](https://travis-ci.org/wez/wezterm)

![Screenshot](screenshots/one.png)

*Screenshot of wezterm on X11, running vim*

## Quickstart

* Install `rustup` to get the `rust` compiler installed on your system.
  https://www.rust-lang.org/en-US/install.html
* Build in release mode: `cargo build --release`
* Run it via either `cargo run --release` or `target/release/wezterm`

You will need a collection of support libraries; the [`get-deps`](get-deps) script will
attempt to install them for you.  If it doesn't know about your system,
[please contribute instructions!](CONTRIBUTING.md)

```
$ curl https://sh.rustup.rs -sSf | sh -s
$ git clone --depth=1 --branch=master https://github.com/wez/wezterm.git
$ cd wezterm
$ sudo ./get-deps
$ cargo build --release
$ cargo run --release
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
- [x] Tabs (Hotkey: `Super-T`, next/prev: `Super-[` and `Super-]`, go-to: `Super-[0-9]`)

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

```
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

### Colors

You can configure colors with a section like this.  In addition to specifying
SVG/CSS3 color names, you can use `#RRGGBB` to specify a color code using the
usual hex notation; eg: `#000000` is equivalent to `black`:

```
[colors]
foreground = "silver"
background = "black"
cursor_bg = "springgreen"
ansi = ["black", "maroon", "green", "olive", "navy", "purple", "teal", "silver"]
brights = ["grey", "red", "lime", "yellow", "blue", "fuchsia", "aqua", "white"]
```

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


