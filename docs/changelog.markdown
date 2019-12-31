## Changes

Releases are named using the date, time and git commit
hash.

### Nightly

A bleeding edge build is produced continually (at least
daily) from the master branch.  It may not be usable and
the feature set may change.  As features stabilize some
brief notes about them may accumulate here.

* [Changed Backspace/Delete handling](https://github.com/wez/wezterm/commit/f0e94084d1df36009b879b06e9cfd2be946168e8)
* Added `MoveTabRelative` for changing the ordering of tabs within a window
  using key assignments `CTRL+SHIFT+PageUp` and `CTRL+SHIFT+PageDown`

### 20191229-193639-e7aa2f3

* Fixed a hang when using middle mouse button to paste
* Recognize 8-bit C1 codes encoded as UTF-8, which are used in the Fedora 31 bash prexec notification for gnome terminal
* Ensure that underlines are a minimum of 1 pixel tall
* Reduced CPU utilization on some Wayland compositors
* Added `$WEZTERM_CONFIG_FILE` to the start of the config file search path
* Added new font rendering options:

```
font_antialias = "Subpixel" # None, Greyscale, Subpixel
font_hinting = "Full" # None, Vertical, VerticalSubpixel, Full
```

* Early startup errors now generate a "toast" notification, giving you more of a clue about what went wrong
* We now use the default configuration if the config file had errors, rather than refusing to start
* Wayland compositors: Improved detection of display scaling on startup
* Added `harfuzz_features` option to specify stylistic sets for fonts such as Fira Code, and to control various typographical options
* Added a `window_padding` config section to add padding to the window display
* We now respect [DECSCUSR and DECTCEM](https://github.com/wez/wezterm/issues/7) escape sequence to select between hidden, block, underline and bar cursor types, as well as blinking cursors.  New configuration options have been added to control the appearance and blink rate.
* We now support an optional basic scroll bar.  The scroll bar occupies the right window padding and has a configurable color.  Scroll bars are not yet supported for multiplexer connections and remain disabled by default for the moment.
* Color scheme changes made in the config file now take effect at config reload time for all tabs that have not applied a dynamic color scheme.

### 20191218-101156-bf35707

* Configuration errors detected during config loading are now shown as a system notification
* New `font_dirs` configuration option to specify a set of dirs to search for fonts. Useful for self-contained wezterm deployments.
* The `font_system` option has been split into `font_locator`, `font_shaper` and `font_rasterizer` options.
* Don't allow child processes to inherit open font files on posix systems!
* Disable Nagle's algorithm for `wezterm ssh` sessions
* Add native Wayland window system support

### 20191124-233250-cb9fd7d

* New tab bar UI displays tabs and allows creating new tabs
* Configuration file changes are hot reloaded and take effect automatically on save
* `wezterm ssh user@host` for ad-hoc SSH sessions. You may also define SSH multiplexer sessions.
* `wezterm serial /dev/ttyUSB0` to connect to your Arduino
* `wezterm imgcat /some/image.png` to display images inline in the terminal using the iTerm2 image protocol
* IME support on macOS and Windows systems
* Automatic fallback to software rendering if no GPU is available (eg: certain types of remote desktop sessions)


