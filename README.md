# Wez's Terminal

A terminal emulator implemented in Rust.

## Quickstart

* Install `rustup` to get the *nightly* `rust` compiler installed on your system.
  https://www.rust-lang.org/en-US/install.html
* Build in release mode: `cargo build --release`
* Run it via either `cargo run --release` or `target/release/wezterm`

You will need a collection of support libraries; important to note is that
your `harfbuzz` library must have support for `hb_ft_font_create_referenced`;
older linux distributions don't have this!

```
$ sudo apt-get install -y libxcb-icccm4-dev libxcb-ewmh-dev \
    libxcb-image0-dev libxcb-keysyms1-dev libharfbuzz-dev \
    libfontconfig1-dev libfreetype6-dev
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

## Status / Features

These are in the done/doing soon category:

- [x] Runs on Linux with XCB
- [x] Scrollback (use mouse wheel and Shift Page{Up|Down})
- [x] True Color support
- [x] Color Emoji and font fallback
- [x] Paste selection via Shift-Insert
- [ ] xterm style selection of text with mouse
- [ ] Configuration file to specify fonts and colors
- [ ] Render underline, italic, bold, strikethrough
- [ ] Command line argument parsing instead of launching user shell

There's a good number of terminal escape sequences that are not yet implemented
and that will get fleshed out as the applications I use uncover them.
Similarly for key mappings.

Things that I'd like to see happen and that have no immediate priority
(contributions to get closer to these are welcomed!)

- [ ] Runs on macOS
- [ ] Tabs
- [ ] Textual renderer.  Think `tmux` or `screen`.
- [ ] Runs on Windows

## Configuration

`wezterm` will look for a TOML configuration file in `$HOME/.config/wezterm/wezterm.toml`,
and then in `$HOME/.wezterm.toml`.

Configuration is currently very simple and the format is considered unstable and subject
to change.  The code for configuration can be found in `src/config.rs`.

I use the following in my `~/.wezterm.toml`:

```
font_size = 10
font = { fontconfig_pattern = "Operator Mono SSm Lig" }
```

The default configuration will attempt to use whichever font is returned from
fontconfig when `monospace` is requested.
