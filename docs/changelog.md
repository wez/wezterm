## Changes

Releases are named using the date, time and git commit
hash.

### Nightly

A bleeding edge build is produced continually (at least
daily) from the master branch.  It may not be usable and
the feature set may change.  As features stabilize some
brief notes about them may accumulate here.

* New: [tab_bar_style](config/lua/config/tab_bar_style.md) allows customizing the appearance of the rest of tha tab bar.
* New: animated gif and png images displayed via `wezterm imgcat` (the iTerm2 image protocol), or attached to the window background via [window_background_image](config/appearance.html#window-background-image) will now animate while the window has focus.
* New: added [foreground_text_hsb](config/lua/config/foreground_text_hsb.md) setting to adjust hue, saturation and brightness when text is rendered.
* New: added [ResetFontAndWindowSize](config/lua/keyassignment/ResetFontAndWindowSize.md) key assignment.
* New: added [ScrollByLine](config/lua/keyassignment/ScrollByLine.md) key assignment.
* New: OSC 777 and OSC 9 escapes now generate Toast Notifications. `printf "\e]777;notify;%s;%s\e\\" "title" "body"` and `printf "\e]9;%s\e\\" "hello there"`.  These don't currently pass through multiplexer connections. [#489](https://github.com/wez/wezterm/issues/489).
* New: [exit_behavior](config/lua/config/exit_behavior.md) config option to keep panes open after the program has completed. [#499](https://github.com/wez/wezterm/issues/499)
* New: added `--config name=value` options to `wezterm`, `wezterm-gui` and `wezterm-mux-server`.  The `--front-end`, `--font-locator`, `--font-rasterizer` and `--font-shaper` CLI options have been removed in favor of this new mechanism.
* New: [window:set_config_overrides](config/lua/window/set_config_overrides.md) method that can be used to override GUI related configuration options on a per-window basis. Click through to see examples of dynamically toggling ligatures and window opacity. [#469](https://github.com/wez/wezterm/issues/469) [#329](https://github.com/wez/wezterm/issues/329)
* New: introduced [custom_block_glyphs](config/lua/config/custom_block_glyphs.md) option to ensure that block glyphs don't have gaps. [#433](https://github.com/wez/wezterm/issues/433)
* New: you can now drag the wezterm window via the tab bar
* New: holding SUPER+Drag (or CTRL+SHIFT+Drag) will drag the wezterm window.  Use [StartWindowDrag](config/lua/keyassignment/StartWindowDrag.md) to configure your own binding.
* New: configure [window_decorations](config/lua/config/window_decorations.md) to remove the title bar and/or window border
* New: we now bundle [PowerlineExtraSymbols](https://github.com/ryanoasis/powerline-extra-symbols) as a built-in fallback font, so that you can use powerline glyphs with any font without patching the font.
* New: [window:set_right_status](config/lua/window/set_right_status.md) allows setting additional status information in the tab bar. [#500](https://github.com/wez/wezterm/issues/500)
* New: Search Mode: Added `CTRL-u` key assignment to clear the current search pattern. Thanks to [@bew](https://github.com/bew)! [#465](https://github.com/wez/wezterm/pull/465)
* Fonts: `font_antialias` and `font_hinting` are now deprecated in favor of the new [freetype_load_target](config/lua/config/freetype_load_target.md) and [freetype_load_flags](config/lua/config/freetype_load_flags.md) options.  The deprecated options have no effect and will be removed in a future release.  The new options provide more direct control over how freetype rasterizes text.
* Fonts: when computing default `font_rules` for bold and italic fonts, strip italic and bold components from the family name. eg: if you set `font = wezterm.font("Source Code Pro Medium")` then the ` Medium` text will be stripped from the font name used to locate bold and italic variants so that we don't report an error loading a non-sensical `Source Code Pro Medium Bold`. [#456](https://github.com/wez/wezterm/issues/456)
* Fonts: fix a regression where bright windows behind wezterm could "shine through" on the alpha channel, and adjust the tinting operation to avoid anti-aliased dark fringes [#470](https://github.com/wez/wezterm/issues/470) [#491](https://github.com/wez/wezterm/issues/491)
* Fonts: macOS: fix an issue where wezterm could hang when loading a font located via Core Text [#475](https://github.com/wez/wezterm/issues/475)
* Fonts: Changed the default [font_size](config/lua/config/font_size.md) to 12 points. [#517](https://github.com/wez/wezterm/discussions/517)
* Fonts: Updated bundled JetBrainsMono font to version 2.225
* Added `--config-file` CLI option to specify an alternate config file location. [Read more about config file resolution](config/files.md). Thanks to [@bew](https://github.com/bew)! [#459](https://github.com/wez/wezterm/pull/459)
* OSC 52 (Clipboard manipulation) now respects the difference between PRIMARY and CLIPBOARD on X11 systems.
* Fixed an issue where large pastes could result in a hang
* Closing the configuration error window no longer requires confirmation
* Fixed: an issue where the window would be redrawn on mouse move. This was most noticeable as a laggy mouse pointer when moving the mouse across a window running on the nouveau display driver on X11 and Wayland systems
* Fixed: an issue where closing a pane would immediately `SIGKILL` the associated process, rather than sending `SIGHUP`.  Thanks to [@bew](https://github.com/bew)!
* Fixed: line-based mouse selection (default: triple click) now extends forwards to include wrapped lines. [#466](https://github.com/wez/wezterm/issues/466)
* Fixed: the [RIS](https://vt100.net/docs/vt510-rm/RIS) escape wasn't clearing the scrollback. [#511](https://github.com/wez/wezterm/issues/511)
* Wayland: fixed opengl context creation issues.  Thanks to [@unrelentingtech](https://github.com/unrelentingtech)! [#481](https://github.com/wez/wezterm/pull/481)
* Wayland: the raw key modifiers are now correctly propagated so that they activate when used with key assignments using the `key = "raw:123"` binding syntax.
* Wayland: fixed window decoration and full screen handling [#224](https://github.com/wez/wezterm/issues/224)
* Wayland: fixed an issue where key repeat processing could "run away" and hang the application
* Windows: the portable .zip file download now includes ANGLE EGL, just like the setup.exe installer has done since version 20201031-154415-9614e117
* Windows: Fixed [ToggleFullScreen](config/lua/keyassignment/ToggleFullScreen.md) so that it once again toggles between full screen and normal placement. [#177](https://github.com/wez/wezterm/issues/177)
* Windows: fix the unexpected default behavior of Ctrl-Alt being converted to AltGr for layouts supporting this key, the previous behavior is still possible by enabling the option [`treat_left_ctrlalt_as_altgr`](config/lua/config/treat_left_ctrlalt_as_altgr.md) (to solve [#392](https://github.com/wez/wezterm/issues/392)). Thanks to [@bew](https://github.com/bew)! [#512](https://github.com/wez/wezterm/pull/512)
* Windows: fixed "Open WezTerm Here" context menu in explorer when used on the root of a drive (eg: `C:\`).  Thanks to [@flyxyz123](https://github.com/flyxyz123)! [#526](https://github.com/wez/wezterm/issues/526) [#451](https://github.com/wez/wezterm/issues/451)
* X11: fix an issue where SHIFT-Enter was not recognized [#516](https://github.com/wez/wezterm/issues/516)
* X11: improved DPI detection for high-DPI displays. [#515](https://github.com/wez/wezterm/issues/515)
* X11: we now load the XCursor themes when possible, which means that the mouse cursor is now generally a bit larger and clearer as well as conforming more with the prevailing style of the desktop environment. [#524](https://github.com/wez/wezterm/issues/524)

### 20210203-095643-70a364eb

* Fix cursor position after using iTerm2 image protocol [#317](https://github.com/wez/wezterm/issues/317)
* Fix pixel dimensions after changing the pane size; this was mostly invisible but impacted image scaling when using sixel or iTerm2 image protocols. [#312](https://github.com/wez/wezterm/issues/312)
* Add support for OSC 133 which allows annotating output as `Output`, `Input` (that you typed) and `Prompt` (shell "chrome"). [Learn more about Semantic prompt and OSC 133](https://gitlab.freedesktop.org/Per_Bothner/specifications/blob/master/proposals/semantic-prompts.md)
* Add [`ScrollToPrompt`](config/lua/keyassignment/ScrollToPrompt.md) key assignment that scrolls the viewport to the prior/next shell prompt emitted using OSC 133 Semantic Prompt escapes.  This assignment is not bound by default.
* Fixed an issue where `SpawnWindow` didn't use the current working directory from the current pane to spawn the new window
* Added `wezterm start --class CLASSNAME` option to specify the window class name under X11 and Windows, or the `app_id` under Wayland.  See `wezterm start --help` for more information.
* Added shell integration for setting OSC 7 (working directory) and OSC 133 (semantic zones) for Zsh and Bash. [See Shell Integration docs](shell-integration.md).
* Added `SemanticZone` as a possible parameter for [SelectTextAtMouseCursor](config/lua/keyassignment/SelectTextAtMouseCursor.md), making it possible to conveniently select complete input or output regions.
* Improved font rendering [#320](https://github.com/wez/wezterm/issues/320) [#331](https://github.com/wez/wezterm/issues/331) [#413](https://github.com/wez/wezterm/issues/413) and changed `font_antialias = "Greyscale"` by default.
* Updated internal harfbuzz shaper to 2.7.2
* Fixed ALT-Escape not sending ESC-ESC [#338](https://github.com/wez/wezterm/issues/338)
* Added `allow_square_glyphs_to_overflow_width = "WhenFollowedBySpace"` option to allow square symbol glyphs to deliberately overflow their specified cell width when the next cell is a space.  Can be set to `Always` to allow overflowing regardless of the next cell being a space, or `Never` to strictly respect the cell width.  The default is `Never`. [#342](https://github.com/wez/wezterm/issues/342)
* macOS: Improved key input when Option is pressed.  Fixed dead key processing when `use_ime=true`. [#357](https://github.com/wez/wezterm/issues/357)
* macOS: Adjusted default dpi to 72 to bring point sizes into alignment with other macOS apps. [#332](https://github.com/wez/wezterm/issues/332)
* Improved font fallback; we now try harder to find a system-provided font for glyphs that are not found in your explicitly configured fonts.
* Revised pty output processing and removed the related `ratelimit_output_bytes_per_second` option
* Workaround Cocoa leaking window position saved state file descriptors to child processes on macOS Big Sur, and Gnome/Mutter doing something similar under X11
* The 256 color cube now uses slightly brighter colors [#348](https://github.com/wez/wezterm/issues/348)
* New: added `line_height` configuration option to scale the computed cell height. The default is `1.0`, resulting in using the font-specified metrics. Setting it to `1.2` will result in a 20% larger cell height.
* macOS: Fixed an issue where hovering over the split between panes could result in wezterm becoming unresponsive [#391](https://github.com/wez/wezterm/issues/391)
* Closing windows and `QuitApplication` will now prompt for confirmation before proceeding with the close/quit.  Added `window_close_confirmation` to control this; valid values are `AlwaysPrompt` and `NeverPrompt`. [#280](https://github.com/wez/wezterm/issues/280)
* Tidied up logging. Previously ERROR level logging was used to make sure that informational things showed up in the stderr stream. Now we use INFO level logging for this to avoid alarming the user.  You can set `WEZTERM_LOG=trace` in the environment to get more verbose logging for troubleshooting purposes.
* Windows: fix an issue where VNC-server-emulated AltGr was not treated as AltGr [#392](https://github.com/wez/wezterm/issues/392)
* X11: fix an issue where keys that produce unicode characters retained SHIFT as a modifier instead of normalizing it away. [#394](https://github.com/wez/wezterm/issues/394)
* Fixed an issue where a symbol-only font would be seen as 0-width and panic wezterm [#404](https://github.com/wez/wezterm/issues/404)
* Tweaked mouse selection: we now round the x-coordinate to the nearest cell which makes it a bit more forgiving if the mouse cursor is slightly to the left of the intended cell start. [#350](https://github.com/wez/wezterm/issues/350)
* Added `selection_word_boundary` option to control double-click word selection boundaries. The default is <tt> \t\n{}\[\]()\"'\`</tt>. [#405](https://github.com/wez/wezterm/issues/405)
* Added support for Curly, Dotted and Dashed underlines.  See [this documentation](faq.html#how-do-i-enable-undercurl-curly-underlines) on the escape sequences how enable undercurl support in vim and nvim. [#415](https://github.com/wez/wezterm/issues/415)
* Fixed an issue where wezterm would spawn processes with `umask 077` on unix systems, rather than the more commonly expected `umask 022`. [#416](https://github.com/wez/wezterm/issues/416)
* macOS: We now ship a Universal binary containing both Intel and "Apple Silicon" architectures
* Setting a really large or really small font scale (using CTRL +/-) no longer causes a panic [#428](https://github.com/wez/wezterm/issues/428)
* Fixed an issue where the mouse wheel wasn't mapped to cursor up/down when the alternate screen was active [#429](https://github.com/wez/wezterm/issues/429)
* Fixed `ToggleFullScreen` not working on macOS and X11.  It still doesn't function on Windows.  `native_macos_fullscreen_mode = false` uses a fast full-screen window on macOS. Set it to `true` to use the slower macOS native "Spaces" style fullscreen mode. [#177](https://github.com/wez/wezterm/issues/177)
* Windows: fix an issue where the initial window size didn't factor the correct DPI when the system-wide display scaling was not 100%. [#427](https://github.com/wez/wezterm/issues/427)
* New: `adjust_window_size_when_changing_font_size` option to control whether changing the font size adjusts the dimensions of the window (true) or adjusts the number of terminal rows/columns (false).  The default is `true`. [#431](https://github.com/wez/wezterm/issues/431)
* macOS: we no longer use MetalANGLE to render the gui; it was short lived as macOS Big Sur now uses Metal in its CGL implementation.  Support for using MetalANGLE is still present if the dylib is found on startup, but we no longer ship the dylib.
* Windows: when pasting text, ensure that the text has CRLF line endings unless bracketed paste is enabled. This imperfect heuristic helps to keep multi-line pastes on multiple lines when using Windows console applications and to avoid interleaved blank lines when using unix applications. [#411](https://github.com/wez/wezterm/issues/411)
* New: [ClearScrollback](config/lua/keyassignment/ClearScrollback.html) now accepts a parameter to control whether the viewport is cleared along with the scrollback. Thanks to [@dfrankland](https://github.com/dfrankland)!
* New: [default_cwd](config/lua/config/default_cwd.html) to specify an alternative current working directory. Thanks to [@dfrankland](https://github.com/dfrankland)!
* New: [CopyTo](config/lua/keyassignment/CopyTo.md) and [PasteFrom](config/lua/keyassignment/PasteFrom.md) actions. [Copy](config/lua/keyassignment/Copy.md), [Paste](config/lua/keyassignment/Paste.md) and [PastePrimarySelection](config/lua/keyassignment/PastePrimarySelection.md) are now deprecated in favor of these new options.
* X11: Mouse-based selection now copies-to and pastes-from the `PrimarySelection` by default. The [CompleteSelection](config/lua/keyassignment/CompleteSelection.md) and [CompleteSelectionOrOpenLinkAtMouseCursor](config/lua/keyassignment/CompleteSelectionOrOpenLinkAtMouseCursor.md) actions now require a parameter to specify the clipboard.
* X11: `SHIFT-CTRL-C` and `SHIFT-CTRL-V` now copy-to and paste from the `Clipboard` by default.  `SHIFT-Insert` pastes from the `PrimarySelection` by default.
* New: Added a new default `CTRL-Insert` key assignment bound to `CopyTo(PrimarySelection)`
* macOS: Windows now have drop-shadows when they are opaque. These were disabled due transparency support was added. Thanks to [Rice](https://github.com/fanzeyi)! [#445](https://github.com/wez/wezterm/pull/445)
* Unix: adjust font-config patterns to also match "dual spacing" fonts such as [Iosevka Term](https://typeof.net/Iosevka/). Thanks to [Leiser](https://github.com/leiserfg)! [#446](https://github.com/wez/wezterm/pull/446)
* New: Added [alternate_buffer_wheel_scroll_speed](config/lua/config/alternate_buffer_wheel_scroll_speed.md) option to control how many cursor key presses are generated by the mouse wheel when the alternate screen is active. The new default for this is a faster-than-previous-releases 3 lines per wheel tick. [#432](https://github.com/wez/wezterm/issues/432)
* macOS: Dead Keys are now processed even when `use_ime=false`.  [More details in the docs](config/keys.md#macos-left-and-right-option-key). [#410](https://github.com/wez/wezterm/issues/410).
* X11: attempt to load cursors from the XCursor.theme resource specified on the root window [#524](https://github.com/wez/wezterm/issues/524)
* Added `file://` URL matching to the default list of implicit hyperlink rules [#525](https://github.com/wez/wezterm/issues/525)

### 20201101-103216-403d002d

* Whoops! fixed a crash on macOS when using multiple windows in the new Metal renderer [#316](https://github.com/wez/wezterm/issues/316)

### 20201031-154415-9614e117

* New: split/pane support! `CTRL+SHIFT+ALT+"` to [SplitVertical](config/lua/keyassignment/SplitVertical.md),
  and `CTRL+SHIFT+ALT+%` to [SplitHorizontal](config/lua/keyassignment/SplitHorizontal.md).
* New: [LEADER](config/keys.md#leader-key) modifier key support
* New: `window_background_opacity` and `window_background_image`
  options to control using background images, transparent windows.
  [More info](config/appearance.md#window-background-image)
* New color schemes: `Dracula+`, `Gruvbox Light`, `MaterialDarker`,
  `Overnight Slumber`, `Popping and Locking`, `Rapture`,
  `jubi`, `nord`.
* New: expanded lua API allows handling URI clicks and keyboard events
  with lua callbacks.  See [wezterm.on](config/lua/wezterm/on.md) docs.
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
  working directory that should be set in newly spawned local panes/tabs,
  in case you don't have OSC 7 integration setup in your shell.
* We now bundle *JetBrains Mono* and use it as the default font,
  and add it as a default fallback font.  Similarly, we also
  bundle *Noto Color Emoji* as a default fallback for emoji.
* Added `automatically_reload_config=false` option to disable
  automatic config reloading.  When set to false, you will need
  to manually trigger a config reload (default: `SUPER+R` or
  `CTRL+SHIFT+R`)
* [`CloseCurrentTab`](config/lua/keyassignment/CloseCurrentTab.md)
  now requires a `confirm` parameter.
* Halved the memory usage requirements per Cell in the common
  case (saving 32 bytes per cell), which gives more headroom for
  users with large scrollback.
* Reduced initial GPU VRAM requirement to 2MiB.  Improved texture
  allocation to avoid needing lots of VRAM.
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
  software renderer if available (X11/Wayland/Windows).
  The old basic CPU renderer has been removed.
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
* Windows: we now prefer to use Direct3D11 to render the
  gui, via [ANGLE](https://chromium.googlesource.com/angle/angle/)
  EGL.  The primary benefit of this is that upgrading your
  graphics drivers while you have a stateful wezterm session
  will no longer terminate the wezterm process. Resize
  behavior is not as smooth with ANGLE as the prior WGL.
  If you wish, you can set `prefer_egl = false` to use
  WGL.
* Improved image protocol support to have better render fidelity
  and to reduce VRAM usage when the same image it displayed
  multiple times in the same pane.

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


