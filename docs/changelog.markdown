## Changes

Releases are named using the date, time and git commit
hash.

### Nightly

A bleeding edge build is produced continually (at least
daily) from the master branch.  It may not be usable and
the feature set may change.  As features stabilize some
brief notes about them may accumulate here.

* New: split/pane support!
  See [#157](https://github.com/wez/wezterm/issues/157) for
  overall status and additional information.
* The GUI layer now normalizes SHIFT state for keyboard processing.
  If a keypress is ASCII uppercase and SHIFT is held then the
  SHIFT modifier is removed from the set of active modifiers.  This
  has implications for your key assignment configuration; previously
  you would write `{key="T", mods="CTRL|SHIFT"}`, after updating to
  this release you need to write `{key="T", mods="CTRL"}` in order
  for your key bindings to take effect.
* Added `show_tab_index_in_tab_bar` option which defaults to true.
  Causes the tab's ordinal index to be prefixed to tab titles.
  The displayed number is 1-based.  You can set
  `tab_and_split_indices_are_zero_based=true` if you prefer the
  number to be zero based.
* On Linux and macOS systems, wezterm can now attempt to guess the current
  working directory that should be set in newly spawned local panes/tabs.
* We now bundle *JetBrains Mono* and use it as the default font,
  and add it as a default fallback font.  Similarly, we also
  bundle *Noto Color Emoji* as a default fallback for emoji.
* Added `automatically_reload_config=false` option to disable
  automatic config reloading.  When set to false, you will need
  to manually trigger a config reload (default: `SUPER+R` or
  `CTRL+SHIFT+R`)
* [`CloseCurrentTab`](config/lua/keyassignment/CloseCurrentTab.md)
  now requires a `confirm` parameter.
* New: [LEADER](config/keys.md#leader-key) modifier key support
* Halved the memory usage requirements per Cell in the common
  case (saving 32 bytes per cell), which gives more headroom for
  users with large scrollback.
* macOS: Fix issue where new windows would open as Cocoa tabs
  when wezterm was maximized.
* macOS: Fix issue where wezterm wouldn't adjust to DPI changes
  when dragging across monitors or the screen resolution changed
* macOS: Reduced trackpad based scrolling sensitivity; it was
  hyper sensitive in previous releases, and now it is more
  reasonable.
* Fix an issue where EGL failed to initialize on Linux
* If EGL/WGL/OpenGL fail to initialize, we now try to fallback
  to Mesa OpenGL in software render mode.  This should result
  in its llvmpipe renderer being used as a fallback, which
  has improved visuals compared to wezterm's own basic CPU
  based renderer.  (This applies to X11/Wayland and Windows
  systems).
* Setting `front_end="Software"` will try to use the Mesa OpenGL
  software renderer.  You can select `OldSoftware` if you
  really want the basic CPU renderer (not recommended; it
  doesn't support panes).
* The multiplexer server has been moved into its own
  `wezterm-mux-server` executable.  You will need to revise
  your `serve_command` configuration.
* Windows: when started in an RDP session, force the use
  of the Mesa software renderer to work around problems with
  RDP GPU emulation.
* Fixed an issue with TLS Multiplexing where bootstrapping
  certificates would usually fail.
* Windows: Fixed an issue that prevented ALT-Space from
  showing the system menu in the window.
* Windows: Fixed dead key handling.  By default dead keys
  behave the same as in other programs and produce diacritics.
  However, setting `use_dead_keys = false` in the config will
  cause dead keys to behave like a regular key; eg: `^` would
  just emit `^` as its own character.
* Windows: Fixed an issue with the `Hide` key assignment;
  it would hide the window with no way to show it again!
  `Hide` now minimizes the window instead.
* macOS: we now use Metal to render the gui, via
  [MetalANGLE](https://github.com/kakashidinho/metalangle)

### 20200909-002054-4c9af461

* Added support for OSC 1 (Icon Title changing), and changed
  how that interacts with OSC 2 (Window Title changing).
  If you specify OSC 1 as a non-empty string, then that will
  be used for the title of that terminal instance in the GUI.
  Otherwise the Window Title will be reported instead.
* Added missing mappings for Application Keypad keys on Linux
* Workaround an EGL issue where Mesa reports the least-best
  alpha value when enumerating configs, rather than the best
  alpha.  This could lead to incorrect alpha under XWayland
  and failure to initialize EGL and fallbacks to the Software
  renderer in some other cases.
* `enable_wayland` now defaults to `false`; mutter keeps breaking
  client-side window decoration so let's just make it opt-in so
  that the default experience is better.
* Fixed a crash on Linux/X11 when using `wezterm connect HOST`
* Added `tab_max_width` config setting to limit the maximum
  width of tabs in the tab bar.  This defaults to 16 glyphs
  in width.

### 20200718-095447-d2315640

* Added support for DECSET 1004 Focus Reporting to local
  (not multiplexer) terminal sessions.
* Added support for SGR 53/55 which enable/disable Overline style.
  `printf "\x1b[53moverline\x1b[0m\n"`
* Windows: updated bundled openconsole.exe to [efb1fdd](https://github.com/microsoft/terminal/commit/efb1fddb991dc1e6b614d1637daca7314a229925)
  to resolve an issue where bold text didn't respect the configured color scheme.
* Added `bold_brightens_ansi_colors` option to allow disabling the automatic
  brightening of bold text.
* Unix: fix an issue where setting the current working directory for a custom
  spawned command would not take effect (thanks @john01dav!)
* Windows: fixed buffering/timing issue where a response to a color query in
  vim could be misinterpreted and replace a character in the editor with the
  letter `g`.
* X11: Improved support for non-24bpp display depths.  WezTerm now tries
  harder to obtain an 8bpc surface on both 16bpp and 30bpp (10bpc) displays.
* Windows: fixed falling back to a simpler OpenGL context if WGL is unable
  to negotiate a robust context.  This is useful on systems with dual
  high/low power GPU hardware where the OpenGL versions for the two GPUs
  are different!
* Color Schemes: synced with [ea2c841](https://github.com/mbadolato/iTerm2-Color-Schemes/commit/ea2c84115d8cff97b5255a7344090902ae669245)
  which includes new schemes: `Adventure`, `Banana Blueberry`, `Blue Matrix`,
  `BlueBerryPie`, `Cyberdyne`, `Django`, `DjangoRebornAgain`, `DjangoSmooth`,
  `DoomOne`, `Konsolas`, `Laser`, `Mirage`, `Rouge 2`, `Sakura`, `Scarlet
  Protocol`, `synthwave-everything`, `Tinacious Design (Dark)`, `Tinacious
  Design (Light)`.

### 20200620-160318-e00b076c

* Fixed default mapping of ambiguous ctrl key combinations (`i`, `m`, `[`, `{`,
  `@`) so that they emit the old school tab, newline, escape etc. values.
  These got broken as part of prototyping CSI-u support a while back.
* Added option to enable CSI-u key encodings.  This is a new mapping scheme
  defined here <http://www.leonerd.org.uk/hacks/fixterms/> that disambiguates
  and otherwise enables more key binding combinations.  You can enable this
  setting using `enable_csi_u_key_encoding = true` in your config file.
* Very early support for sixel graphics
* macos: `use_ime` now defaults to false; this is a better out of
  the box experience for most users.
* macos: we now attempt to set a reasonable default LANG environment based
  on the locale settings at the time that wezterm is launched.
* macos: introduce `send_composed_key_when_left_alt_is_pressed` and
  `send_composed_key_when_right_alt_is_pressed` boolean config settings.  Like
  the existing `send_composed_key_when_alt_is_pressed` option, these control
  whether the `Alt` or `Option` modifier produce composed output or generate
  the raw key position with the ALT modifier applied.  The difference from the
  existing config option is that on systems where Left and Right Alt can be
  distinguished you now have the ability to control this behavior
  independently.  The default behavior on these systems is
  `send_composed_key_when_left_alt_is_pressed=false` and
  `send_composed_key_when_right_alt_is_pressed=true` so that the right Alt key
  behaves more like an `AltGr` key and generates the composed input, while the
  Left Alt is regular uncomposed Alt.
* Fonts: fixed an issue where specifying italic or bold in the second parameter
  of `wezterm.font` didn't work as intended or documented
* Improved terminal emulation conformance; added left/right margin support
  and now passes [esctest](https://gitlab.freedesktop.org/terminal-wg/esctest)
  to a similar degree as iTerm2
* Fixed an issue where unmodified F5+ would use the CSI-u encoded-modifiers
  format, and confused eg: `htop`.
* `ActivateTab` now accepts negative numbers as a way to reference the last
  tab in the Window.  The default assignment for `CTRL+SHIFT+9` and `CMD+9`
  is now `ActivateTab=-1`, which selects the last tab.
* Fixed an issue when applying hyperlink rules to lines that had mixed width
  characters

### 20200607-144723-74889cd4

* Windows: Fixed AltGr handling for European layouts
* X11: Added `PastePrimarySelection` key assignment that pastes the contents
  of the primary selection rather than the clipboard.
* Removed old TOML config file parsing code
* Removed old `arg="something"` key binding parameter.  This was a remnant from
  the TOML based configuration.  You're unlikely to notice this unless you
  followed an example from the docs; migrate instead to using eg:
  `action=wezterm.action{ActivateTab=i-1}` to pass the integer argument.
* Windows: now also available with a setup.exe installer.  The installer
  enables "Open WezTerm Here" in the explorer.exe context menu.
* Added `ClearScrollback` key assignment to clear the scrollback.  This is bound to CMD-K and CTRL-SHIFT-K by default.
* Added `Search` key assignment to search the scrollback.  Read the new
  [scrollback](scrollback.html) section for more information!
* Fixed an issue where ALT+number would send the wrong output for European
  keyboard layouts on macOS and Linux.  As part of this the default behavior
  has changed: we used to force ALT+number to produce ALT+number instead of
  the composed key for that layout.  We now emit the composed key by default.
  You can switch to the old behavior either by explicitly binding those keys
  or by setting `send_composed_key_when_alt_is_pressed = false` in your
  configuration file.
* Windows: the launcher menu now automatically lists out any WSL environments
  you have installed so that you can quickly spawn a shell in any of them.
  You can suppress this behavior if you wish by setting
  `add_wsl_distributions_to_launch_menu = false`.
  [Read more about the launcher menu](config/launch.html#the-launcher-menu)
* Added `ActivateCopyMode` key assignment to put the tab into mouseless-copy
  mode; [use the keyboard to define the selected text region](copymode.html).
  This is bound to CTRL-SHIFT-X by default.

### 20200517-122836-92c201c6

* AppImage: Support looking for configuration in `WezTerm.AppImage.config` and
  `WezTerm.AppImage.home` to support portable thumbdrive use of wezterm on
  linux systems
* We now check the github releases section for updated stable releases and show
  a simple UI to let you know about the update, with links to download/install
  it.  We don't automatically download the release: just make a small REST API
  call to github.  There is no data collection performed by the wezterm project
  as part of this.  We check once every 24 hours.  You can set
  `check_for_updates = false` in your config to disable this completely if
  desired, or set `check_for_updates_interval_seconds` to an alternative update
  interval.
* Added support for OSC 110-119 to reset dynamic colors, improving our support for Neovim.
* Change OSC rendering to use the long-form `ST` sequence `ESC \` rather than
  the more convenient alternative `BEL` representation, which was not
  recognized by Neovim when querying for color information.
* Fixed Shift-Tab key on X11 and Wayland
* WezTerm is now also available to Windows users via [Scoop](https://scoop.sh/)

### 20200503-171512-b13ef15f

* Added the `launch_menu` configuration for the launcher menu
  as described in [Launching Programs](config/launch.html).
* Fixed a crash when reloading a config with `enable_tab_bar=false`
* Fixed missing icon when running under X11 and Wayland
* Wayland client-side-decorations improved and now also render window title
* Implicitly SGR reset when switching alt and primary screen
* Improved config error reporting UI: we now show just a single
  window with all errors rather than one window per failed reload.

### 20200406-151651-5b700e4

* Added lua based configuration.  Reading TOML configuration will be rapidly
  phased out in favor of the more flexible lua config; for now, both are
  supported, but new features may not be available via TOML.
* Added launcher overlay.  Right click the `+` button on the tab bar or
  bind a key to `ShowLauncher` to activate it.  It allows spawning tabs in
  various domains as well as attaching multiplexer sessions that were not
  connected automatically at startup.
* Windows: we now support mouse reporting on Windows native ptys.  For this to
  work, `conpty.dll` and `OpenConsole.exe` must be present alongside `wezterm.exe`
  when starting wezterm.
* Added `initial_rows` and `initial_cols` config options to set the starting
  size of new terminal windows
* Added `hide_tab_bar_if_only_one_tab = true` config option to hide the tab
  bar when the window contains only a single tab.
* Added `HideApplication` key action (defaults to `CMD-H` on macOS only) which
  hides the wezterm application.  This is macOS specific.
* Added `QuitApplication` key action which causes the gui loop to terminate
  and the application to exit.  This is not bound by default, but you may
  choose to assign it to something like `CMD-Q`.
* Added `set_environment_variables` configuration section to allow defining
  some environment variables to be passed to your shell.
* Improved connectivity UI that shows ssh and mux connection progress/status
* Fixed a bug where the baud rate was not applied when opening a serial port
* Added predictive local echo to the multiplexer for higher latency connections
* We now grey out the UI for lagging multiplexer connections
* Set an upper bound on the memory usage for multiplexer connections


### 20200202-181957-765184e5

* Improved font shaping performance 2-3x by adding a shaper cache
* Windows: now has support for TLS based multiplexer connections
* Multiplexer: TLS multiplexer can now be bootstrapped via SSH, and automatically
  manages certificates
* Unix: We now default to spawning shells with the `-l` argument to request a login
  shell.  This is important on macOS where the default GUI environment doesn't
  source a working PATH from the shell, resulting in an anemic PATH unless the
  user has taken care to cover this in their shell startup.  `-l` works to enable
  a login shell in `zsh`, `bash`, `fish` and `tcsh`.  If it doesn't work with your
  shell, you can use the `default_prog` configuration option to override this.
* We now accept `rgb:XX/XX/XX` color syntax for OSC 4 and related escape
  sequences; previously only `#XXXXXX` and named colors were accepted.
* We now accept OSC 104 to reset custom colors to their defaults.
* Added Tab Navigator overlay for folks that hoard tabs; it presents
  an interactive UI for selecting and activating a tab from a vertically
  oriented list.  This is bound to `Alt-9` by default.
* Added support for DEC Origin Mode (`DECOM`) which improves cursor positioning
  with some applications
* Added support for DEC AutoWrap Mode (`DECAWM`) which was previously always on.
  This improves rendering for applications that explicitly disable it.
* We now show a connection status window while establishing MUX and SSH connections.
  The status window is also where any interactive authentication is carried out
  for eg: SSH sessions.
* Improved SSH authentication handling; we now give you a few opportunities to
  authenticate and are now able to successfully authenticate with sites that
  have configured 2-Factor authentication in their server side SSH configuration.
* Fixed an issue where SHIFT-Space would swallow the space key.
* Nightly builds are now available for Linux in [AppImage](https://github.com/wez/wezterm/releases/download/nightly/WezTerm-nightly.AppImage) format.
* Shift+Left Mouse button can now be used to extend the selection to the clicked location.  This is particularly helpful when you want to select something that is larger than the viewport.
* Windows: a single mouse wheel tick now scrolls by the number of positions configured in the Windows system settings (default 3)
* Windows: fixed IME position when the tab bar is enabled
* Windows: removed support for WinPty, which was too difficult to obtain, configure and use.
* Configuration errors now show in a separate window on startup, or when the configuration is reloaded
* Improved reliability and performance of MUX sessions, although they still have room for further improvement


### 20200113-214446-bb6251f

* Added `color_scheme` configuration option and more than 200 color schemes
* Improved resize behavior; lines that were split due to
  the width of the terminal are now rewrapped on resize.
  [Issue 14](https://github.com/wez/wezterm/issues/14)
* Double-click and triple-click and hold followed by a drag now extends
  the selection by word and line respectively.
* The OSC 7 (CurrentWorkingDirectory) escape sequence is now supported; wezterm records the cwd in a tab and that will be used to set the working directory when spawning new tabs in the same domain.  You will need to configure your shell to emit OSC 7 when appropriate.
* [Changed Backspace/Delete handling](https://github.com/wez/wezterm/commit/f0e94084d1df36009b879b06e9cfd2be946168e8)
* Added `MoveTabRelative` for changing the ordering of tabs within a window
  using key assignments `CTRL+SHIFT+PageUp` and `CTRL+SHIFT+PageDown`
* [The multiplexer protocol is undergoing major changes](https://github.com/wez/wezterm/issues/106).
  The multiplexer will now raise an error if the client and server are incompatible.
* Fixed an issue where wezterm would linger for a few seconds after the last tab was closed
* Fixed an issue where wezterm wouldn't repaint the screen after a tab was closed
* Clicking the OS window close button in the titlebar now closes the window rather than the active tab
* Added `use_ime` option to optionally disable the use of the IME on macOS.  You might consider enabling this if you don't like the way that the IME swallows key repeats for some keys.
* Fix an [issue](https://github.com/knsd/daemonize/pull/39) where the pidfile would leak into child processes and block restarting the mux server
* Fix an issue where the title bars of remote tabs were not picked up at domain attach time
* Fixed selection and scrollbar position for multiplexer tabs
* Added `ScrollByPage` key assignment and moved the `SHIFT+PageUp` handling up to the
  gui layer so that it can be rebound.
* X11: a single mouse wheel tick now scrolls by 5 rows rather than 1
* Wayland: normalize line endings to unix line endings when pasting
* Windows: fixed handling of focus related messages, which impacted both the appearance of
  the text cursor and copy and paste handling.
* When hovering over implicitly hyperlinked items, we no longer show the underline for every other URL with the same destination

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


