## Changes

Releases are named using the date, time and git commit hash.

### Continuous/Nightly

A bleeding edge build is produced continually (as commits are made, and at
least a daily scheduled build) from the `main` branch.  It *may* not be usable
and the feature set may change, but since @wez uses this as a daily driver, its
usually the best available version.

As features stabilize some brief notes about them will accumulate here.

#### New
* Copy Mode now supports using `CTRL-u` and `CTRL-d` to move by half a page at
  a time. Thanks to [@pengux](https://github.com/pengux)!
  [#2662](https://github.com/wez/wezterm/pull/2662)
* macOS: allow association with `.command`, `.sh`, `.zsh`, `.bash`, `.fish` and
  `.tool` scripts, so that those can open and show their output in the
  terminal. [#2871](https://github.com/wez/wezterm/issues/2871)
  [#2741](https://github.com/wez/wezterm/issues/2741)
* macOS: initial cut at macOS native menu bar
  [#1485](https://github.com/wez/wezterm/issues/1485)
* mux: exposed [MuxDomain](config/lua/MuxDomain/index.md) to lua, along with
  [wezterm.mux.get_domain()](config/lua/wezterm.mux/get_domain.md),
  [wezterm.mux.all_domains()](config/lua/wezterm.mux/all_domains.md) and
  [wezterm.mux.set_default_domain()](config/lua/wezterm.mux/set_default_domain.md).
* [hide_mouse_cursor_when_typing](config/lua/config/hide_mouse_cursor_when_typing.md)
  option to control whether the mouse cursor is hidden when typing. Thanks to
  [@ProspectPyxis](https://github.com/ProspectPyxis)!
  [#2946](https://github.com/wez/wezterm/pull/2946)
* [pane:get_text_from_region()](config/lua/pane/get_text_from_region.md),
  [pane:get_text_from_semantic_zone()](config/lua/pane/get_text_from_semantic_zone.md),
  [pane:get_semantic_zones()](config/lua/pane/get_semantic_zones.md),
  [pane:get_semantic_zone_at()](config/lua/pane/get_semantic_zone_at.md)
* Color schemes: [Apple Classic](colorschemes/a/index.md#apple-classic),
  [\_bash (Gogh)](colorschemes/b/index.md#bash-gogh),
  [Breath (Gogh)](colorschemes/b/index.md#breath-gogh),
  [BreathLight (Gogh)](colorschemes/b/index.md#breathlight-gogh),
  [BreathSilverfox (Gogh)](colorschemes/b/index.md#breathsilverfox-gogh),
  [Breeze (Gogh)](colorschemes/b/index.md#breeze-gogh),
  [Everblush](colorschemes/e/index.md#everblush),
  [EverforestDark (Gogh)](colorschemes/e/index.md#everforestdark-gogh),
  [EverforestLight (Gogh)](colorschemes/e/index.md#everforestlight-gogh),
  [GruvboxDark](colorschemes/g/index.md#gruvboxdark),
  [GruvboxDarkHard](colorschemes/g/index.md#gruvboxdarkhard),
  [kanagawa (Gogh)](colorschemes/k/index.md#kanagawa-gogh),
  [rose-pine](colorschemes/r/index.md#rose-pine),
  [rose-pine-dawn](colorschemes/r/index.md#rose-pine-dawn),
  [rose-pine-moon](colorschemes/r/index.md#rose-pine-moon)
* [window:focus()](config/lua/window/focus.md),
  [ActivateWindow](config/lua/keyassignment/ActivateWindow.md),
  [ActivateWindowRelative](config/lua/keyassignment/ActivateWindowRelative.md),
  [ActivateWindowRelativeNoWrap](config/lua/keyassignment/ActivateWindowRelativeNoWrap.md)
* Copy Mode: added
  [MoveForwardWordEnd](config/lua/keyassignment/CopyMode/MoveForwardWordEnd.md),
  thanks to [@GZLiew](https://github.com/GZLiew)!
  [#2908](https://github.com/wez/wezterm/pull/2908)
* [tab:get_size()](config/lua/MuxTab/get_size.md),
  [tab:rotate_counter_clockwise()](config/lua/MuxTab/rotate_counter_clockwise.md).
  [tab:rotate_counter_clockwise()](config/lua/MuxTab/rotate_counter_clockwise.md).
* [wezterm.config_builder()](config/lua/wezterm/config_builder.md)
* [gui-attached](config/lua/gui-events/gui-attached.md) event provides some
  more flexibility at startup.
* [wezterm cli get-text](cli/cli/get-text.md) command for capturing the content of a pane.

#### Fixed
* X11: hanging or killing the IME could hang wezterm
  [#2819](https://github.com/wez/wezterm/issues/2819)
* `wezterm ssh` now respects the `AddressFamily` option when connecting
  [#2893](https://github.com/wez/wezterm/issues/2893)
* Windows: panic when minimizing a window when `front_end='WebGpu'`
  [#2881](https://github.com/wez/wezterm/issues/2881)
* X11: `wezterm.screens().active` is now populated based on the currently
  focused window, rather than just copying the `main` screen data. Thanks to
  [@NBonaparte](https://github.com/NBonaparte)!
  [#2928](https://github.com/wez/wezterm/pull/2928)
* Resizing windows when overriding the dpi in the config would not respect
  the specified dpi. Thanks to [@niclashoyer](https://github.com/niclashoyer)!
  [#2914](https://github.com/wez/wezterm/issues/2914)
  [#2978](https://github.com/wez/wezterm/pull/2978)
* Wayland: mouse cursor didn't auto-hide when typing. Thanks to
  [@jmbaur](https://github.com/jmbaur)!
  [#2977](https://github.com/wez/wezterm/pull/2977)
* mux: `default_workspace` was not always respected when spawning
  [#2981](https://github.com/wez/wezterm/issues/2981)
* [window:active_key_table()](config/lua/window/active_key_table.md) now
  factors in pane-specific key table stacks for things like `CopyMode`.
  [#2986](https://github.com/wez/wezterm/discussions/2986)
* modal overlays like CharSelect and the command palette sometimes wouldn't
  render when first activated until pressing a key.
* lag when making heavy use of foreground process information in tab titles.
  [#2991](https://github.com/wez/wezterm/issues/2991)
* X11: always update selection owner even if our window already owns it
  [#2926](https://github.com/wez/wezterm/issues/2926)
* command output would not be displayed if the command exited almost
  immediately after starting up.
* Windows: ALT key was not recognized when processing mouse events.
  Thanks to [@mbikovitsky](https://github.com/mbikovitsky)!
  [#3053](https://github.com/wez/wezterm/pull/3053)
* Copy Mode:
  [MoveForwardWord](config/lua/keyassignment/CopyMode/MoveForwardWord.md) not
  always moving to next line.  thanks to [@GZLiew](https://github.com/GZLiew)!
  [#2955](https://github.com/wez/wezterm/pull/2955)

#### Changed
* `CTRL-SHIFT-P` now activates the new command palette, instead of `PaneSelect`
  [#1485](https://github.com/wez/wezterm/issues/1485)
* Window title reporting escape sequences are now disabled by default.
  [See here for more details](https://marc.info/?l=bugtraq&m=104612710031920&w=2)
* Withdraw DEC private SGR escapes that affect superscript and
  subscript due to xterm/vim conflict
  [mintty/#1189](https://github.com/mintty/mintty/issues/1189)
* Removed deprecated `Copy`, `Paste` and `PastePrimarySelection` actions. Use
  [CopyTo](config/lua/keyassignment/CopyTo.md) and
  [PasteFrom](config/lua/keyassignment/PasteFrom.md) instead.
* `wezterm -e` is now an alias for `wezterm start`. Thanks to
  [@Abdiramen](https://github.com/Abdiramen)!
  [#2889](https://github.com/wez/wezterm/pull/2889)
  [#2782](https://github.com/wez/wezterm/issues/2782)
* [bold_brightens_ansi_colors](config/lua/config/bold_brightens_ansi_colors.md)
  now supports `"BrightOnly"` to use the bright color without selecting a bold
  font. [#2932](https://github.com/wez/wezterm/issues/2932)
* Color schemes: `Gruvbox Dark` was renamed to `GruvboxDark` and adjusted in
  the upstream iTerm2-Color-Schemes repo
* Config warnings, such as using deprecated or invalid fields will now cause
  the configuration error window to be shown. Previously, only hard errors were
  shown, which meant that a number of minor config issues could be overlooked.
* Referencing `wezterm.GLOBAL` now returns references rather than copies, making
  it less cumbersome to code read/modify/write with global state
* `wezterm start` now accepts `--domain` and `--attach` options. `wezterm
  connect DOMAIN` is now implemented internally as `wezterm start --domain
  DOMAIN --attach`.
* X11: spurious pointer focus events no longer influence terminal focus events.
  Thanks to [@NBonaparte](https://github.com/NBonaparte)!
  [#2959](https://github.com/wez/wezterm/pull/2959)

#### Updated
* Bundled harfbuzz updated to version 6.0.0

### 20221119-145034-49b9839f

#### Improved
* Reduced CPU and RAM utilization, reduced overhead of parsing output and
  rendering to the GPU.

#### New
* [wezterm.gui.default_key_tables](config/lua/wezterm.gui/default_key_tables.md)
  and [wezterm.gui.default_keys](config/lua/wezterm.gui/default_keys.md) for
  more conveniently copying and extending the default configuration.
* [normalize_output_to_unicode_nfc](config/lua/config/normalize_output_to_unicode_nfc.md)
  option to normalize terminal output to Unicode NFC prior to applying it to
  the terminal model.  [#2482](https://github.com/wez/wezterm/issues/2482)
* [cursor_thickness](config/lua/config/cursor_thickness.md),
  [underline_thickness](config/lua/config/underline_thickness.md),
  [underline_position](config/lua/config/underline_position.md) and
  [strikethrough_position](config/lua/config/strikethrough_position.md) options
  to fine tune appearance. [#2505](https://github.com/wez/wezterm/issues/2505)
  [#2326](https://github.com/wez/wezterm/issues/2326)
* Support for `modifyOtherKeys` keyboard encoding
  [#2527](https://github.com/wez/wezterm/issues/2527)
* Superscript and subscript text attributes via SGR 73 and SGR 74
* [wezterm cli activate-pane-direction](cli/cli/activate-pane-direction.md)
  command. Thanks to [@abusch](https://github.com/abusch)!
  [#2526](https://github.com/wez/wezterm/pull/2526)
* [window:is_focused()](config/lua/window/is_focused.md) method for testing
  whether a GUI window has focus.
  [#2537](https://github.com/wez/wezterm/discussions/2537)
* [window-focus-changed](config/lua/window-events/window-focus-changed.md)
  event.
* [pane:inject_output](config/lua/pane/inject_output.md) method
* [ResetTerminal](config/lua/keyassignment/ResetTerminal.md) key assignment
* Support for Utf8 mouse reporting (DECSET 1005).
  [#2613](https://github.com/wez/wezterm/issues/2613)
* [ActivateKeyTable](config/lua/keyassignment/ActivateKeyTable.md) now also
  supports `prevent_fallback = true` as a parameter.
  [#2702](https://github.com/wez/wezterm/issues/2702)
* [show_tabs_in_tab_bar](config/lua/config/show_tabs_in_tab_bar.md) and
  [show_new_tab_button_in_tab_bar](config/lua/config/show_new_tab_button_in_tab_bar.md)
  config options to customize the tab bar appearance.
  [#2082](https://github.com/wez/wezterm/issues/2082)

#### Fixed
* Wayland: key repeat gets stuck after pressing two keys in quick succession.
  Thanks to [@valpackett](https://github.com/valpackett)!
  [#2492](https://github.com/wez/wezterm/pull/2492)
  [#2452](https://github.com/wez/wezterm/issues/2452)
* If the underline attribute was active and CRLF scrolled a new line into the
  bottom of the display, we'd fill that new line with underlines.
  [#2489](https://github.com/wez/wezterm/issues/2489)
* Correctly invalidate the display when using
  `wezterm.action.ClearScrollback("ScrollbackAndViewport")`
  [#2498](https://github.com/wez/wezterm/issues/2498)
* Hyperlinks didn't underline on hover
  [#2496](https://github.com/wez/wezterm/issues/2496)
* base16 color schemes cursor fg/bg were the same. We now also set the indexed
  colors.  Thanks to [@valpackett](https://github.com/valpackett)!
  [#2491](https://github.com/wez/wezterm/pull/2492)
* Panic when processing a sixel with inconsistent width information
  [#2500](https://github.com/wez/wezterm/issues/2500)
* Cells with the invisible/hidden attribute are now invisible
* Panic when trying to activate the search overlay when the launcher menu is
  active [#2529](https://github.com/wez/wezterm/issues/2529)
* Overlays did not see config overrides set via `window:set_config_overrides`
  [#2544](https://github.com/wez/wezterm/issues/2544)
* Closing a window while tab had a zoomed pane would leave the other panes
  untouched and wezterm would linger in the background
  [#2548](https://github.com/wez/wezterm/issues/2548)
* CharSelect panic when pressing enter when no matches were found
  [#2580](https://github.com/wez/wezterm/issues/2580)
* Panic when setting `initial_rows` or `initial_cols` to `0`
  [#2593](https://github.com/wez/wezterm/issues/2593)
* X11: Crash on systems using DRI2 based Intel graphics
  [#2559](https://github.com/wez/wezterm/issues/2559)
* Missing validation of conflicting domain names
  [#2618](https://github.com/wez/wezterm/issues/2618)
* Creating tabs in a multiplexing domain could fail after previously closing
  all tabs connected to that domain in that window
  [#2614](https://github.com/wez/wezterm/issues/2614)
* CharSelect now uppercases hex digit input for better compatibility with
  QMK-based keyboards that send eg: `CTRL-SHIFT-U e 1 <ENTER>`.
  [#2581](https://github.com/wez/wezterm/issues/2581)
* Multiple active multiplexer client domain connections could result
  in showing duplicate tabs in a window
  [#2616](https://github.com/wez/wezterm/issues/2616)
* Incorrect line width when applying hyperlink rules to a wrapped line
  containing double-wide cells.
  [#2568](https://github.com/wez/wezterm/issues/2568)
* Incorrect shaping for U+28 U+FF9F
  [#2572](https://github.com/wez/wezterm/issues/2572)
* Panic when hitting enter in launcher menu when no fuzzy results match
  [#2629](https://github.com/wez/wezterm/issues/2629)
* Default `CTRL-SHIFT-<NUM>` assignments didn't work on Windows and X11
  systems when `key_map_preference = "Mapped"`
  [#2623](https://github.com/wez/wezterm/issues/2623)
* Panic when calling `window:set_workspace` when the default domain is a
  multiplexer domain.
  [#2638](https://github.com/wez/wezterm/issues/2638)
* nvim's `title` and `titlestring` options don't work when `TERM=wezterm`.
  [#2112](https://github.com/wez/wezterm/issues/2112)
* Horizontal wheel scrolling generated incorrect mouse events
  [#2649](https://github.com/wez/wezterm/issues/2649)
* Cursor color changes did not always render
  [#2708](https://github.com/wez/wezterm/issues/2708)
* Unable to set cursor on Wayland/X11
  [#2687](https://github.com/wez/wezterm/issues/2687)
  [#2743](https://github.com/wez/wezterm/issues/2743)
* Default `MoveTabRelative` assignments were incorrectly set to
  `SUPER+SHIFT+Page(Up|Down)` instead of the documented
  `CTRL+SHIFT+Page(Up|Down)`
  [#2705](https://github.com/wez/wezterm/issues/2705)
* Dragging by retro tab bar left or right status area would jump around erratically.
  [#2758](https://github.com/wez/wezterm/issues/2758)
* Fixed background `Cover` algorithm. Thanks to
  [@xiaopengli89](https://github.com/xiaopengli89)!
  [#2636](https://github.com/wez/wezterm/pull/2636)
* `wezterm start --cwd .` didn't use the cwd of the spawned process when the
  wezterm gui was already running. Thanks to
  [@exactly-one-kas](https://github.com/exactly-one-kas)!
  [#2661](https://github.com/wez/wezterm/pull/2661)
* IME composition text and cursor color incorrectly applied to all panes rather
  than just the active pane.
  [#2569](https://github.com/wez/wezterm/issues/2569)

#### Changed
* Removed Last Resort fallback font
* X11: use `_NET_WM_MOVERESIZE` to drag by tab bar, when supported by the WM
  [#2530](https://github.com/wez/wezterm/issues/2530)
* `tab:panes()` and `tab:panes_with_info()` now return the full list of panes
  in the tab regardless of whether a pane was zoomed. Previously, if a pane was
  zoomed, only that pane would be returned by those methods.
* macOS: CTRL-modified keys are now routed to the IME
  [#2435](https://github.com/wez/wezterm/pull/2435)
* multiplexer: The lag indicator that gets overlaid on the pane content
  when waiting a long time for a response now defaults to disabled.  It is
  recommended that you [put it into your status
  bar](config/lua/pane/get_metadata.md), but you may re-enable the old way
  using `overlay_lag_indicator = true` in the appropriate domain
  configuration.
* Added dummy `-e` command line option to support programs that assume that all
  terminal emulators support a `-e` option. Thanks to
  [@vimpostor](https://github.com/vimpostor)!.
  [#2670](https://github.com/wez/wezterm/pull/2670)
  [#2622](https://github.com/wez/wezterm/issues/2622)
  [#2271](https://github.com/wez/wezterm/issues/2271)
* Windows: installer no longer prevents installing the x64 binary on arm64 systems.
  The x64 executable is installed and run via emulation.
  Thanks to [@xeysz](https://github.com/xeysz)!
  [#2746](https://github.com/wez/wezterm/pull/2746)
  [#2667](https://github.com/wez/wezterm/issues/2667)

#### Updated
* Bundled Nerd Font Symbols font to v2.2.2
* Bundled harfbuzz to 5.3.1

### 20220905-102802-7d4b8249

#### New
* [switch_to_last_active_tab_when_closing_tab](config/lua/config/switch_to_last_active_tab_when_closing_tab.md)
  option to control behavior when closing the active tab.
  [#2487](https://github.com/wez/wezterm/issues/2487)
#### Changed
* fontconfig: when locating a fallback font for a given codepoint, allow
  matching non-monospace fonts if we can't find any matching monospace fonts.
  [#2468](https://github.com/wez/wezterm/discussions/2468)
* `os.getenv` now knows how to resolve environment variables that would normally
  require logging out to update, such as `SHELL` (if you `chsh` on unix systeams),
  or those set through the registry on Windows. [#2481](https://github.com/wez/wezterm/discussions/2481)
* Searching is now incremental and shows progress. [#1209](https://github.com/wez/wezterm/issues/1209)

#### Fixed
* Hangul in NFD incorrectly shaped [#2482](https://github.com/wez/wezterm/issues/2482)
* Visual artifacts when resizing splits [#2483](https://github.com/wez/wezterm/issues/2483)

### 20220904-064125-9a6cee2b

* Fix build on architectures where `c_char` is `u8` instead of `i8`. Thanks to [@liushuyu](https://github.com/liushuyu)! [#2480](https://github.com/wez/wezterm/pull/2480)

### 20220903-194523-3bb1ed61

#### New

* Color schemes: [carbonfox](colorschemes/c/index.md#carbonfox), [DanQing Light (base16)](colorschemes/d/index.md#danqing-light-base16), [Dracula (Official)](colorschemes/d/index.md#dracula-official), [Poimandres](colorschemes/p/index.md#poimandres), [Poimandres Storm](colorschemes/p/index.md#poimandres-storm), [Sequoia Monochrome](colorschemes/s/index.md#sequoia-monochrome), [Sequoia Moonlight](colorschemes/s/index.md#sequoia-moonlight), [SynthwaveAlpha](colorschemes/s/index.md#synthwavealpha), [SynthwaveAlpha (Gogh)](colorschemes/s/index.md#synthwavealpha-gogh)
* [window_frame](config/lua/config/window_frame.md) now supports setting border size and color [#2417](https://github.com/wez/wezterm/issues/2417)
* [CopyMode](copymode.md) now supports selecting and move by semantic zones. [#2346](https://github.com/wez/wezterm/issues/2346)
* [max_fps](config/lua/config/max_fps.md) option to limit maximum frame rate [#2419](https://github.com/wez/wezterm/discussions/2419)
* [`user-var-changed` event](config/lua/window-events/user-var-changed.md) allows triggering lua code in response to user vars being changed
* `CTRL-SHIFT-U` activates a new Emoij/Unicodes/NerdFont character picker modal overlay. Fuzzy search by name or hex unicode codepoint value, or browse with keys. `CTRL-r` to cycle the browser between categories. `Enter` to select an item, copy it to the clipboard and send it to the active pane as input. `Esc` to cancel. [CharSelect](config/lua/keyassignment/CharSelect.md).
* `CTRL-SHIFT-P` is now a default assignment for [PaneSelect](config/lua/keyassignment/PaneSelect.md)
* Cursor now changes to a lock glyph to indicate when local echo is disabled for password entry. Detection is limited to local unix processes and cannot work with tmux. Use `detect_password_input=false` to disable this. [#2460](https://github.com/wez/wezterm/issues/2460)

#### Changed

* `colors` now override colors from your selected `color_scheme`. Previously, `color_scheme` was mutually exclusive with `colors` and always took precedence. The new behavior is more in line with what most people expect.
* Reduced CPU utilization for busy/large screen updates, blinking cursor and other easing animations
* [ActivatePaneDirection](config/lua/keyassignment/ActivatePaneDirection.md) now uses recency to resolve ambiguous moves [#2374](https://github.com/wez/wezterm/issues/2374)
* [update-status](config/lua/window-events/update-status.md) is a more general event for updating left or right status. `update-right-status` is considered to be deprecated in favor of `update-status`.
* Cache XDG Portal Appearance values. Thanks to [@vimposter](https://github.com/vimpostor)! [#2402](https://github.com/wez/wezterm/pull/2402)
* Compensate for TUI programs that flicker due to unsynchronized output by adding up to 3ms additional latency after each read to coalesce their screen outputs into a single frame. You can set this delay via a new `mux_output_parser_coalesce_delay_ms` option. [#2443](https://github.com/wez/wezterm/issues/2443)
* win32: Updated openconsole/conpty to v1.14.2281.0

#### Fixed

* macOS: crash on startup if `$SHELL` points to something that isn't executable. [#2378](https://github.com/wez/wezterm/issues/2378)
* tab titles truncated too short [#2379](https://github.com/wez/wezterm/issues/2379)
* `bypass_mouse_reporting_modifiers` stopped working (regression around new mouse binding logic) [#2389](https://github.com/wez/wezterm/issues/2389)
* Entering IME-composed text would have no effect in `wezterm ssh` [#2434](https://github.com/wez/wezterm/issues/2434)
* `gui-startup` event now also works with `wezterm ssh`
* `x` and `+` buttons in the fancy tab bar are now always square [#2399](https://github.com/wez/wezterm/issues/2399)
* middle clicking a tab to close it will now confirm closing using the same rules as [CloseCurrentTab](config/lua/keyassignment/CloseCurrentTab.md) [#2350](https://github.com/wez/wezterm/issues/2350)
* Emitting the tmux-style `ESC k TITLE ST` sequence via ConPTY breaks output for the pane [#2442](https://github.com/wez/wezterm/issues/2442)
* Avoid using full path canonicalization for `--cwd` options [#2449](https://github.com/wez/wezterm/issues/2449)
* Scroll to the bottom on mouse input when mouse reporting is enabled [#2447](https://github.com/wez/wezterm/issues/2447)
* ssh: correctly expand `%h` ssh_config tokens [#2448](https://github.com/wez/wezterm/issues/2448)
* ssh: `CloseCurrentPane` wouldn't release all resources associated with the pane and could lead to a `too many open files` error for a long running `wezterm ssh` session. [#2466](https://github.com/wez/wezterm/issues/2466)
* mouse cursor is now reset to arrow when the mouse leaves the window [#2471](https://github.com/wez/wezterm/issues/2471)

### 20220807-113146-c2fee766

#### New
* [ActivateKeyTable](config/lua/keyassignment/ActivateKeyTable.md) now supports `until_unknown=true` to implicitly pop the table when a key not defined by that table is pressed. [#2178](https://github.com/wez/wezterm/issues/2178)
* [window:copy_to_clipboard](config/lua/window/copy_to_clipboard.md) method for putting arbitrary text into the clipboard/selection.
* [window:set_inner_size](config/lua/window/set_inner_size.md) method for controlling window size.
* [window:set_position](config/lua/window/set_position.md) method for controlling window position.
* [window:maximize](config/lua/window/maximize.md) and [window:restore](config/lua/window/restore.md) methods for controlling window maximization state.
* [window:get_selection_escapes_for_pane](config/lua/window/get_selection_escapes_for_pane.md) method for getting the current selection including escape sequences. [#2223](https://github.com/wez/wezterm/issues/2223)
* [window:current_event](config/lua/window/current_event.md) method for getting the current event. [#2296](https://github.com/wez/wezterm/pull/2296)
* [wezterm.color](config/lua/wezterm.color/index.md) module for working with colors and importing color schemes.
* [wezterm.gui](config/lua/wezterm.gui/index.md) module and [mux_window:gui_window](config/lua/mux-window/gui_window.md) method.
* [wezterm.gui.screens()](config/lua/wezterm.gui/screens.md) function for getting information about the available screens/monitors/displays
* [wezterm.gui.get_appearance()](config/lua/wezterm.gui/get_appearance.md) function for a simpler way to get system dark mode state
* [wezterm.procinfo](config/lua/wezterm.procinfo/index.md) module for querying local process information.
* [wezterm.time](config/lua/wezterm.time/index.md) module for working with time, including methods for determining sun rise/set.
* You may now use [wezterm.format](config/lua/wezterm/format.md) (or otherwise use strings with escape sequences) in the labels of the [Launcher Menu](config/launch.md#the-launcher-menu).
* You may now specify `assume_emoji_presentation = true` (or `false`) in [wezterm.font()](config/lua/wezterm/font.md) and [wezterm.font_with_fallback()](config/lua/wezterm/font_with_fallback.md)
* Wayland: `zwp_text_input_v3` is now supported, which enables IME to work in wezterm if your compositor also implements this protocol.
* [wezterm.json_parse()](config/lua/wezterm/json_parse.md) and [wezterm.json_encode()](config/lua/wezterm/json_encode.md) functions for working with JSON.
* Hundreds of new color schemes have been imported from [base16](https://github.com/chriskempson/base16-schemes-source), [Gogh](https://gogh-co.github.io/Gogh/) and [terminal.sexy](https://terminal.sexy/). [Browse the schemes](colorschemes/index.md) and look for themes with `(base16)`, `(Gogh)` and `(terminal.sexy)` in the name to discover them!
* [pane:is_alt_screen_active()](config/lua/pane/is_alt_screen_active.md) for testing whether the alt screen is active. Thanks to [@Funami580](https://github.com/Funami580)! [#2234](https://github.com/wez/wezterm/issues/2234)
* X11/Wayland: [XDG desktop portal](https://flatpak.github.io/xdg-desktop-portal/) is now used to determine whether dark mode is in use [#2258](https://github.com/wez/wezterm/issues/2258)
* [SetPaneZoomState](config/lua/keyassignment/SetPaneZoomState.md) key assignment and [MuxTab:set_zoomed()](config/lua/MuxTab/set_zoomed.md) for explicitly setting the zoom state of a pane. [#2284](https://github.com/wez/wezterm/discussions/2284)
* [mouse_bindings](config/mouse.md) can now handle scroll events. Thanks to [@Funami580](https://github.com/Funami580)! [#2173](https://github.com/wez/wezterm/issues/2173) [#2296](https://github.com/wez/wezterm/pull/2296)
* [mouse_bindings](config/mouse.md) may now also be defined based on whether the alt-screen is active and/or whether the application in the pane has enabled mouse reporting. [#581](https://github.com/wez/wezterm/issues/581)
* `wezterm.action.CopyMode('ClearSelectionMode')` allows clearing the selection mode without leaving [Copy Mode](copymode.md). Thanks to [@aznhe21](https://github.com/aznhe21)! [#2352](https://github.com/wez/wezterm/pull/2352)
* [window:set_left_status](config/lua/window/set_left_status.md) for setting status to the left of the tabs in the tab bar [#1561](https://github.com/wez/wezterm/issues/1561)

#### Changed
* If `timeout_milliseconds` is specified in
  [ActivateKeyTable](config/lua/keyassignment/ActivateKeyTable.md), then the
  timeout duration is now reset each time a key press matches that key table
  activation. [#1129](https://github.com/wez/wezterm/issues/1129)
* The lua examples in the docs are now syntax checked and formatted via
  [Gelatyx](https://github.com/azzamsa/gelatyx) and
  [StyLua](https://github.com/JohnnyMorganz/StyLua), thanks to
  [@azzamsa](https://github.com/azzamsa)!
  [#2273](https://github.com/wez/wezterm/issues/2273)
  [#2253](https://github.com/wez/wezterm/issues/2253)
* Internal scrollback datastructure improvements reduce per-cell overhead by up to ~40x depending on the composition of the line (lines with lots of varied attributes or image attachments will have more overhead).
* Improved search performance
* Quickselect: now defaults to searching 1000 lines above and below the current viewport, making it faster and the labels shorter for users with a larger scrollback. A new `scope_lines` parameter to [QuickSelectArgs](config/lua/keyassignment/QuickSelectArgs.md) allows controlling the search region explicitly. Thanks to [@yyogo](https://github.com/yyogo) for the initial PR! [#1317](https://github.com/wez/wezterm/pull/1317)
* OSC 10, 11 and 12 (Set Default Text Background, Default Text Foreground Color, and Text Cursor Color) now support setting the alpha component [#2313](https://github.com/wez/wezterm/issues/2313), and added [CSI 38:6](escape-sequences.md#csi-386---foreground-color-rgba), `CSI 48:6` and `CSI 58:6` extensions to allow setting full color RGB with Alpha channel for spans of text.
* Copy Mode: setting the same selection mode a second time will now toggle off that mode and clear the selection, preserving the current position [#2246](https://github.com/wez/wezterm/discussions/2246)
* Copy Mode: new default vim-style `y` "yank" key assignment will copy the selection and close copy mode

#### Fixed
* [ActivateKeyTable](config/lua/keyassignment/ActivateKeyTable.md)'s `replace_current` field was not actually optional. Made it optional. [#2179](https://github.com/wez/wezterm/issues/2179)
* `winget` causes toast notification spam [#2185](https://github.com/wez/wezterm/issues/2185)
* `wezterm connect sshdomain` could hang on startup if password authentication was required [#2194](https://github.com/wez/wezterm/issues/2194)
* `colors.indexed` would error out with `Cannot convert String to u8`. [#2197](https://github.com/wez/wezterm/issues/2197)
* X11: closing a window when multiple were open could result in an X protocol error that closed all windows [#2198](https://github.com/wez/wezterm/issues/2198)
* Config will now automatically reload after error. Previously, you would need to manually reload the config using [ReloadConfiguration](config/lua/keyassignment/ReloadConfiguration.md). [#1174](https://github.com/wez/wezterm/issues/1174)
* Config will now automatically reload for changes made to `require`d lua files. Previously, only the main config file and any files that you explicitly passed to [add_to_config_reload_watch_list](config/lua/wezterm/add_to_config_reload_watch_list.md) would trigger a reload.
* macOS: numeric keypad enter generated CTRL-C instead of enter. Regression of [#739](https://github.com/wez/wezterm/issues/739). [#2204](https://github.com/wez/wezterm/issues/2204)
* Wayland: inconsistent pasting. Thanks to [@Funami580](https://github.com/Funami580)! [#2225](https://github.com/wez/wezterm/issues/2225) [#2226](https://github.com/wez/wezterm/pulls/2226)
* win32 input mode: fixed encoding of backspace and delete keys. Thanks to [@kreudom](https://github.com/kreudom)! [#2233](https://github.com/wez/wezterm/pull/2233)
* Tab bar could glitch and show incorrect contents when adjusting for monitor or changed font scaling [#2208](https://github.com/wez/wezterm/issues/2208)
* Wayland: transparent gap under tab bar when window is transparent, split and using per-pane color schemes [#1620](https://github.com/wez/wezterm/issues/1620)
* Tab bar could show a gap to the right when resizing
* Padding could show window background rather than pane background around split panes at certain window sizes [#2210](https://github.com/wez/wezterm/issues/2210)
* Loading dynamic escape sequence scripts from the [iTerm2-Color-Scheme dynamic-colors directory](https://github.com/mbadolato/iTerm2-Color-Schemes/tree/master/dynamic-colors) would only apply the first 7 colors
* Unix: Clicking a URL when no browser is open could cause wezterm to hang until the newly opened browser is closed. [#2245](https://github.com/wez/wezterm/issues/2245)
* Quickselect: now selects the bottom-most match rather than the top-most match. [#2250](https://github.com/wez/wezterm/issues/2250)
* Mux: `wezterm.mux.set_active_workspace` didn't update the current window to match the newly activated workspace. [#2248](https://github.com/wez/wezterm/issues/2248)
* Overlays such as debug and launcher menu now handle resize better
* Shift-F1 through F4 generated different encoding than xterm [#2263](https://github.com/wez/wezterm/issues/2263)
* X11/Wayland: apps that extract the `Exec` field from wezterm.desktop (such as thunar, Dolphin and others) can now simply concatenate the command line they want to invoke, and it will spawn in the their current working directory. Thanks to [@Anomalocaridid](https://github.com/Anomalocaridid)! [#2271](https://github.com/wez/wezterm/pull/2271) [#2103](https://github.com/wez/wezterm/issues/2103) 
* [gui-startup](config/lua/gui-events/gui-startup.md) now passes a [SpawnCommand](config/lua/SpawnCommand.md) parameter representing the `wezterm start` command arguments.
* Tab `x` button is no longer obscured by tab title text for long tab titles [#2269](https://github.com/wez/wezterm/issues/2269)
* Cursor position could end up in the wrong place when rewrapping lines and the cursor was on the rewrap boundary [#2162](https://github.com/wez/wezterm/issues/2162)
* Two or more panes closing at the same time could result in their containing tab hanging and being stuck with "no pane" for a title [#2304](https://github.com/wez/wezterm/issues/2304)
* Visual Bell now fills out to the adjacent window edges rather than being constrained by the padding. [#2364](https://github.com/wez/wezterm/issues/2364)

#### Updated
* Bundled harfbuzz to 5.1.0

### 20220624-141144-bd1b7c5d

#### New
* [background](config/lua/config/background.md) option for rich background compositing and parallax scrolling effects.
* Added [docs for the cli](cli/general.md)
* Support for the [Kitty Keyboard Protocol](https://sw.kovidgoyal.net/kitty/keyboard-protocol). Use [enable_kitty_keyboard](config/lua/config/enable_kitty_keyboard.md)`=true` to enable it.
* New [wezterm.mux](config/lua/wezterm.mux/index.md) module, [gui-startup](config/lua/gui-events/gui-startup.md) and [mux-startup](config/lua/mux-events/mux-startup.md) events for spawning programs into your preferred arrangement when wezterm starts. [#674](https://github.com/wez/wezterm/issues/674)
* ssh client now supports `BindAddress`. Thanks to [@gpanders](https://github.com/gpanders)! [#1875](https://github.com/wez/wezterm/pull/1875)
* [PaneInformation.domain_name](config/lua/PaneInformation.md) and [pane:get_domain_name()](config/lua/pane/get_domain_name.md) which return the name of the domain with which a pane is associated. [#1881](https://github.com/wez/wezterm/issues/1881)
* You may now use `CTRL-n` and `CTRL-p` (in addition to the up/down arrow and vi motion keys) to change the selected row in the Launcher.  Thanks to [@Junnplus](https://github.com/Junnplus)! [#1880](https://github.com/wez/wezterm/pull/1880)
* Attaching multiplexer domains now attaches the first window as a tab in the active window, rather than opening a new window. [#1874](https://github.com/wez/wezterm/issues/1874)
* [AttachDomain](config/lua/keyassignment/AttachDomain.md) and [DetachDomain](config/lua/keyassignment/DetachDomain.md) key assignments
* Specifying a domain name in a [SpawnCommand](config/lua/SpawnCommand.md) will cause that domain to be attached if it is in the detached state. This is useful when combined with [SwitchToWorkspace](config/lua/keyassignment/SwitchToWorkspace.md).
* X11: wezterm now sets `_NET_WM_NAME` in addition to `WM_NAME` for clients that don't know how to fallback
* [treat_east_asian_ambiguous_width_as_wide](config/lua/config/treat_east_asian_ambiguous_width_as_wide.md) for control over how ambiguous width characters are resolved. [#1888](https://github.com/wez/wezterm/issues/1888)
* [clean_exit_codes](config/lua/config/clean_exit_codes.md) config to fine tune [exit_behavior](config/lua/config/exit_behavior.md) [#1889](https://github.com/wez/wezterm/issues/1889)
* [ClearSelection](config/lua/keyassignment/ClearSelection.md) key assignment [#1900](https://github.com/wez/wezterm/issues/1900)
* `wezterm cli list --format json` and `wezterm cli list-clients --format json` allow retrieving data in json format. Thanks to [@ratmice](https://github.com/ratmice)! [#1911](https://github.com/wez/wezterm/pull/1911)
* macOS, Windows, Wayland: you may now drag and drop files from other programs and have their paths paste into the terminal. The new [quote_dropped_files](config/lua/config/quote_dropped_files.md) option controls how the file names are quoted. Thanks to [@junnplus](https://github.com/junnplus), [@datasone](https://github.com/datasone) and [@Funami580](https://github.com/Funami580)! [#1868](https://github.com/wez/wezterm/pull/1868) [#1953](https://github.com/wez/wezterm/pull/1953) [#2148](https://github.com/wez/wezterm/pull/2148)
* The mouse scroll wheel now cycles between tabs when hovering over the tab. Thanks to [@junnplus](https://github.com/junnplus)! [#1726](https://github.com/wez/wezterm/issues/1726)
* Holding down `ALT` while dragging the left button will select a rectangular block. It is also possible to use `ALT+SHIFT` to select a rectangular block. [ExtendSelectionToMouseCursor](config/lua/keyassignment/ExtendSelectionToMouseCursor.md) and [SelectTextAtMouseCursor](config/lua/keyassignment/SelectTextAtMouseCursor.md) now accept `"Block"` as a selection mode. Thanks to [@Funami580](https://github.com/Funami580) for helping! [#1361](https://github.com/wez/wezterm/issues/1361)
* In Copy Mode, `CTRL-v` will enable rectangular block selection mode. [#1656](https://github.com/wez/wezterm/issues/1656)
* In Copy Mode, `SHIFT-v` will enable line selection mode. Thanks to [@bew](https://github.com/bew)! [#2086](https://github.com/wez/wezterm/pull/2086)
* In Copy Mode, `o` and `O` can be used to move the cursor to the other end of the selection, as in vim. Thanks to [@bew](https://github.com/bew)! [#2150](https://github.com/wez/wezterm/pull/2150)
* Copy Mode: key assignments are [now configurable](copymode.md#configurable-key-assignments) [#993](https://github.com/wez/wezterm/issues/993)
* Search Mode: key assignments are [now configurable](scrollback.md#configurable-key-assignments) [#993](https://github.com/wez/wezterm/issues/993)
* Search Mode: the default `CTRL-SHIFT-F` key assignment now defaults to the new `CurrentSelectionOrEmptyString` mode to search for the current selection text, if any.  See [Search](config/lua/keyassignment/Search.md) for more info.
* Copy Mode and Search Mode can be toggled and remember search results and cursor positioning, making it easier to locate and select text without using the mouse [#1592](https://github.com/wez/wezterm/issues/1592)
* In the Launcher Menu, you may now use `CTRL-G` to cancel/exit the launcher [#1977](https://github.com/wez/wezterm/issues/1977)
* [cell_width](config/lua/config/cell_width.md) option to adjust the horizontal spacing when the availble font stretches are insufficient. [#1979](https://github.com/wez/wezterm/issues/1979)
* [min_scroll_bar_height](config/lua/config/min_scroll_bar_height.md) to control the minimum size of the scroll bar thumb [#1936](https://github.com/wez/wezterm/issues/1936)
* [RotatePanes](config/lua/keyassignment/RotatePanes.md) key assignment for re-arranging the panes in a tab
* [SplitPane](config/lua/keyassignment/SplitPane.md) key assignment that allows specifying the size and location of the split, as well as top-level (full width/height) splits. `wezterm cli split-pane --help` shows equivalent options you can use from the cli. [#578](https://github.com/wez/wezterm/issues/578)
* [ime_preedit_rendering](config/lua/config/ime_preedit_rendering.md) option to choose whether to use the builtin or the system IME preedit rendering mode. Thanks to [@kumattau](https://github.com/kumattau)! [#2006](https://github.com/wez/wezterm/pull/2006)
* [wezterm.strftime_utc](config/lua/wezterm/strftime_utc.md) for manipulating times in UTC rather than the local timezone
* `wezterm cli send-text --no-paste` option to send text to a pain without wrapping it as a bracketed paste
* [PaneSelect](config/lua/keyassignment/PaneSelect.md) key assignment to activate the pane selection UI to activate or swap the selected pane. [#1842](https://github.com/wez/wezterm/issues/1842) [#1975](https://github.com/wez/wezterm/issues/1975)
* [window_background_gradient](config/lua/config/window_background_gradient.md) now also supports `Linear` gradients with an angle of your choice. Thanks to [@erf](https://github.com/erf)! [#2038](https://github.com/wez/wezterm/pull/2038)
* RPM and DEB packages now install zsh and bash `wezterm` CLI completions
* Color schemes: [arcoiris](colorschemes/a/index.md#arcoiris), [duckbones](colorschemes/d/index.md#duckbones), [Grey-green](colorschemes/g/index.md#grey-green), [kanagawabones](colorschemes/k/index.md#kanagawabones), [Neon](colorschemes/n/index.md#neon), [neobones_dark](colorschemes/n/index.md#neobones_dark), [neobones_light](colorschemes/n/index.md#neobones_light), [seoulbones_dark](colorschemes/s/index.md#seoulbones_dark), [seoulbones_light](colorschemes/s/index.md#seoulbones_light), [tokyonight-day](colorschemes/t/index.md#tokyonight-day), [tokyonight-storm](colorschemes/t/index.md#tokyonight-storm), [tokyonight](colorschemes/t/index.md#tokyonight), [vimbones](colorschemes/v/index.md#vimbones), [zenbones](colorschemes/z/index.md#zenbones), [zenbones_dark](colorschemes/z/index.md#zenbones_dark), [zenbones_light](colorschemes/z/index.md#zenbones_light), [zenburned](colorschemes/z/index.md#zenburned), [zenwritten_dark](colorschemes/z/index.md#zenwritten_dark), [zenwritten_light](colorschemes/z/index.md#zenwritten_light)
* [wezterm.GLOBAL](config/lua/wezterm/GLOBAL.md) for persisting lua data across config reloads
* `wezterm show-keys` command to show key and mouse binding assignments [#2134](https://github.com/wez/wezterm/issues/2134)

#### Updated
* Bundled harfbuzz to 4.3.0

#### Changed
* Debian packages now register wezterm as an alternative for `x-terminal-emulator`. Thanks to [@xpufx](https://github.com/xpufx)! [#1883](https://github.com/wez/wezterm/pull/1883)
* Windows: wezterm will now read the default environment variables from the `HKLM\System\CurrentControlSet\Control\Session Manager\Environment` and `HKCU\Environment` and apply those to the base environment prior to applying `set_environment_variables`. [#1848](https://github.com/wez/wezterm/issues/1848)
* [Key Table](config/key-tables.md) lookups will now keep searching the activation stack until a matching assignment is found, allowing for layered key tables. [#993](https://github.com/wez/wezterm/issues/993)
* Search mode's search term is now remembered per-tab between activations of search mode. [#1912](https://github.com/wez/wezterm/issues/1912)
* Quickselect no longer jumps to the bottom of the viewport when activated, allowing you to quickselect within the current viewport region
* Quickselect now supports multi-line anchors such as `^` and `$`.  [#2008](https://github.com/wez/wezterm/issues/2008)
* Overriding config using the cli `--config` option will now error out and prevent starting up if unknown config options are specified, or if the value evaluates to `nil`. Unknown options continue to generate warnings (rather than errors) when observed in the config file so that you're not "locked out" of wezterm if you make a typo in the config file.
* Windows: [allow_win32_input_mode](config/lua/config/allow_win32_input_mode.md) now defaults to `true` and enables using [win32-input-mode](https://github.com/microsoft/terminal/blob/main/doc/specs/%234999%20-%20Improved%20keyboard%20handling%20in%20Conpty.md) to send high-fidelity keyboard input to ConPTY. This means that win32 console applications, such as [FAR Manager](https://github.com/FarGroup/FarManager) that use the low level `INPUT_RECORD` API will now receive key-up events as well as events for modifier-only key presses. [#1509](https://github.com/wez/wezterm/issues/1509) [#2009](https://github.com/wez/wezterm/issues/2009) [#2098](https://github.com/wez/wezterm/issues/2098) [#1904](https://github.com/wez/wezterm/issues/1904)
* Wayland: [enable_wayland](config/lua/config/enable_wayland.md) now defaults to `true`. [#2104](https://github.com/wez/wezterm/issues/2104)
* [exit_behavior](config/lua/config/exit_behavior.md) now defaults to `"Close"`. [#2105](https://github.com/wez/wezterm/issues/2105)
* Improved [wezterm.action](config/lua/wezterm/action.md) syntax for slightly more ergnomic and understandable key assignments. [#1150](https://github.com/wez/wezterm/issues/1150)

#### Fixed
* Flush after replying to `XTGETTCAP`, `DECRQM`, `XTVERSION`, `DA2`, `DA3` [#2060](https://github.com/wez/wezterm/issues/2060) [#1850](https://github.com/wez/wezterm/issues/1850) [#1950](https://github.com/wez/wezterm/issues/1950)
* macOS: `CMD-.` was treated as `CTRL-ESC` [#1867](https://github.com/wez/wezterm/issues/1867)
* macOS: `CTRL-Backslash` on German layouts was incorrect [#1891](https://github.com/wez/wezterm/issues/1891)
* `nf-mdi-contacts` nerdfont symbol treated as zero-width [#1864](https://github.com/wez/wezterm/issues/1864)
* X11/Wayland: `CTRL-i`, `CTRL-j`, `CTRL-m` misinterpreted as `CTRL-Tab`, `CTRL-Enter`, `CTRL-Return` [#1851](https://github.com/wez/wezterm/issues/1851)
* Scrollbar stopped working after a lot of output scrolled outside of the scrollback limit.  Thanks to [@davidrios](https://github.com/davidrios)! [#1866](https://github.com/wez/wezterm/pull/1866)
* Windows: uncommitted IME composition could get stuck when switching input methods. [#1922](https://github.com/wez/wezterm/issues/1922)
* OSC sequences, such as those that change the window or tab title, that accept a single string parameter will now join multiple parameters together. This allows window and tab titles to contain a semicolon. Thanks to [@kumattau](https://github.com/kumattau)! [#1939](https://github.com/wez/wezterm/pull/1939)
* Unable to use `ALT` as a modifier for the `leader` key. [#1958](https://github.com/wez/wezterm/issues/1958)
* IME Candidate window position was incorrect. Thanks to [@kumattau](https://github.com/kumattau) and [@aznhe21](https://github.com/aznhe21)! [#1976](https://github.com/wez/wezterm/pull/1976) [#2001](https://github.com/wez/wezterm/pull/2001) [#2022](https://github.com/wez/wezterm/pull/2022)
* Prevent panic for some classes of invalid input, found while fuzzing. Thanks to [@5225225](https://github.com/5225225)! [#1990](https://github.com/wez/wezterm/pull/1990) [#1986](https://github.com/wez/wezterm/pull/1986)
* Detaching an ssh multiplexer domain sometimes killed the associated panes! [#1993](https://github.com/wez/wezterm/issues/1993)
* `DecreaseFontSize` wasn't quite the inverse of `IncreaseFontSize`. Thanks to [@Funami580](https://github.com/Funami580)! [#1997](https://github.com/wez/wezterm/pull/1997)
* Wayland: unable to paste text that was copied before starting the initial wezterm window. Thanks to [@Funami580](https://github.com/Funami580)! [#1994](https://github.com/wez/wezterm/pull/1994) [#1385](https://github.com/wez/wezterm/issues/1385)
* Unicode NFD text could incorrectly render with repeated glyphs [#2032](https://github.com/wez/wezterm/issues/2032)
* Windows: spawning new panes/tabs wouldn't automatically use the working directory of the current pane when OSC 7 was not being used [#2036](https://github.com/wez/wezterm/issues/2036)
* Wayland: panic when display scaling is enabled. [#1727](https://github.com/wez/wezterm/issues/1727)
* `Dark+` color scheme background color [#2013](https://github.com/wez/wezterm/pull/2013)
* Synthesized bold didn't kick in for automatically computed `font_rules`. [#2074](https://github.com/wez/wezterm/issues/2074)
* Added [freetype_pcf_long_family_names](config/lua/config/freetype_pcf_long_family_names.md) option to workaround PCF font naming issues on certain Linux distributions. [#2100](https://github.com/wez/wezterm/issues/2100)
* X11/Wayland: wezterm.desktop now specifies `StartupWMClass`. Thanks to [@uncomfyhalomacro](https://github.com/uncomfyhalomacro)! [#2052](https://github.com/wez/wezterm/issues/2052) [#2125](https://github.com/wez/wezterm/pull/2125)
* `sudo -i` in a pane would cause subsequent pane/tab creation to fail until the cwd was changed to an accessible directory [#2120](https://github.com/wez/wezterm/issues/2120)
* X11: Fixed an issue where some events could get lost around resize events, especially prevalent when using the NVIDIA proprietary drivers. Thanks to [@pjones123](https://github.com/pjones123) and [@yuttie](https://githug.com/yuttie) for working through this! [#1992](https://github.com/wez/wezterm/issues/1992) [#2063](https://github.com/wez/wezterm/issues/2063) [#2111](https://github.com/wez/wezterm/pull/2111) [#1628](https://github.com/wez/wezterm/issues/1628)
* macOS: `SHIFT-Tab` and `CTRL-SHIFT-Tab` produced incorrect results [#1902](https://github.com/wez/wezterm/issues/1902)
* X11: Fixed an issue where copy and paste between two wezterm windows could produce stale results. [#2110](https://github.com/wez/wezterm/issues/2110)
* Mouse selection spanning multiple lines always included the first column even when the mouse was to the left of the first column. Thanks to [@Funami580](https://github.com/Funami580)! [#2106](https://github.com/wez/wezterm/pull/2106)
* Fonts: Codepoints for eg: symbol glyphs that were not explicitly listed in your font fallback list may not be resolved when used in styled (eg: bold) text. [#1913](https://github.com/wez/wezterm/issues/1913) [#2158](https://github.com/wez/wezterm/issues/2158)

### 20220408-101518-b908e2dd

#### New
* [Key Tables](config/key-tables.md) feature for powerful modal key assignments
* `wezterm start --position x,y`, `wezterm start --position displayname:30%,30%` option to control starting window position on all systems except for Wayland. See `wezterm start --help` for more info. [#1794](https://github.com/wez/wezterm/issues/1794)
#### Changed
* Default key assignments are `mapped:` again. A new [key_map_preference](config/lua/config/key_map_preference.md) option allows the defaults to use `"Mapped"` or `"Physical"`.
* Disabled ligatures for `"Monaco"` and `"Menlo"` fonts, as those have `"fi"` ligatures that match even for words such as `find`. [#1786](https://github.com/wez/wezterm/issues/1786) [#1736](https://github.com/wez/wezterm/issues/1736)
* Removed the `send_composed_key_when_alt_is_pressed` option. When processing generic `ALT` (eg: that has neither left nor right), if either `send_composed_key_when_left_alt_is_pressed` or `send_composed_key_when_right_alt_is_pressed` is true, then the composed form of the key event will be generated.
#### Updated and Improved
* Bundled harfbuzz to 4.2.0
* On macOS, non-native fullscreen mode now attempts to avoid the notch on systems that have one. [#1737](https://github.com/wez/wezterm/issues/1737)
* Sixel parsing performance has been improved
* You may now [specify a scaling factor per fallback font](config/lua/wezterm/font_with_fallback.md#manual-fallback-scaling), which is useful when your preferred CJK font renders smaller than your Roman primary font, for example.
* Color schemes: [Retro](colorschemes/r/index.md#retro), [GitHub Dark](colorschemes/g/index.md#github-dark), [Blazer](colorschemes/b/index.md#blazer)
* Wayland: touchpad scroll is now more responsive/precise. Thanks to [@davidrios](https://github.com/davidrios)! [#1800](https://github.com/wez/wezterm/pull/1800) [#1840](https://github.com/wez/wezterm/pull/1840)
* Kitty image protocol: now also supports shared memory data transmission. Thanks to [@tantei3](https://github.com/tantei3)! [#1810](https://github.com/wez/wezterm/pull/1810)
* Secondary DA response bumped up to persuade vim to set `ttymouse=sgr` by default. [#1825](https://github.com/wez/wezterm/issues/1825)

#### Fixed
* Incorrect csi-u encoding with non-ascii characters. [#1746](https://github.com/wez/wezterm/issues/1746)
* X11 `_NET_WM_ICON` had red/blue channels swapped [#1754](https://github.com/wez/wezterm/issues/1754)
* ls-fonts output didn't quote the `style` field [#1762](https://github.com/wez/wezterm/issues/1762)
* `window_decorations = "RESIZE"` on Windows prevented minimize/maximize and aerosnap, double click to maximize, and had an ugly top border. Many thanks to [@davidrios](https://github.com/davidrios)! [#1674](https://github.com/wez/wezterm/issues/1674) [#1675](https://github.com/wez/wezterm/pull/1675) [#1771](https://github.com/wez/wezterm/pull/1771)
* On Windows, explorer shortcut icons with the maximized setting would fall out of maximized state on startup. Thanks to [@davidrios](https://github.com/davidrios)! [#1502](https://github.com/wez/wezterm/issues/1502)
* `LANG` environment variable was not always set on macOS, leading to mojibake when entering CJK.  [#1761](https://github.com/wez/wezterm/issues/1761) [#1765](https://github.com/wez/wezterm/issues/1765)
* Fonts with only non-unicode names (eg: only using a Chinese multibyte string encoding) were treated as having names like `?????` and were not accessible. [#1761](https://github.com/wez/wezterm/issues/1761)
* Hover state of leftmost retro style tab was overly sticky when the mouse moved out of the tab. [#1764](https://github.com/wez/wezterm/issues/1764)
* On macOS, the font size could incorrectly double or halve after waking from sleep or moving the window to/from an external monitor. [#1566](https://github.com/wez/wezterm/issues/1566) [#1745](https://github.com/wez/wezterm/issues/1745)
* On Windows, touchpad scrolling was janky. Many thanks to [@davidrios](https://github.coim/davidrios)! [#1773](https://github.com/wez/wezterm/pull/1773) [#1725](https://github.com/wez/wezterm/pull/1725) [#949](https://github.com/wez/wezterm/pull/949)
* X11: workaround i3-gaps not sending initial CONFIGURE_NOTIFY or FOCUS events, leading to weird initial window size and broken focus status. [#1710](https://github.com/wez/wezterm/issues/1710) [#1757](https://github.com/wez/wezterm/issues/1757)
* Hyperlink rules with more captures than replacements could panic wezterm when text matched. [#1780](https://github.com/wez/wezterm/issues/1780)
* Malformed XTGETTCAP response. [#1781](https://github.com/wez/wezterm/issues/1781)
* Multiplexer performance with images was unusuable for all but tiny images. [#1237](https://github.com/wez/wezterm/issues/1237)
* `CloseCurrentPane{confirm=false}` would leave behind a phantom tab/pane when used with the multiplexer. [#1277](https://github.com/wez/wezterm/issues/1277)
* `CloseCurrentPane{confirm=true}` artifacts when used with the multiplexer. [#783](https://github.com/wez/wezterm/issues/783)
* Scrollbar thumb could jump around/move out of bounds. Thanks to [@davidrios](https://github.com/davidrios)! [#1525](https://github.com/wez/wezterm/issues/1525)
* OSC 52 could stop working for tabs/panes spawned into the GUI via the CLI. [#1790](https://github.com/wez/wezterm/issues/1790)
* Workaround for fonts with broken horizontal advance metrics [#1787](https://github.com/wez/wezterm/issues/1787)
* Improved mouse based selection. Thanks to [@davidrios](https://github.com/davidrios)! [#1805](https://github.com/wez/wezterm/issues/1805) [#1199](https://github.com/wez/wezterm/issues/1199) [#1386](https://github.com/wez/wezterm/issues/1386) [#354](https://github.com/wez/wezterm/issues/354)
* X11 `KP_End` wasn't recognized [#1804](https://github.com/wez/wezterm/issues/1804)
* fontconfig matches now also treat `"charcell"` spacing as monospace. [#1820](https://github.com/wez/wezterm/issues/1820)
* Multiplexer render update laggy, especially when using multiple windows. [#1814](https://github.com/wez/wezterm/issues/1814) [#1841](https://github.com/wez/wezterm/issues/1841)

### 20220319-142410-0fcdea07

#### New

* [window:composition_status()](config/lua/window/composition_status.md) and [window:leader_is_active()](config/lua/window/leader_is_active.md) methods that can help populate [window:set_right_status()](config/lua/window/set_right_status.md) [#686](https://github.com/wez/wezterm/issues/686)
* You may now use `colors = { compose_cursor = "orange" }` to change the cursor color when IME, dead key or leader key composition states are active.
* Support for SGR-Pixels mouse reporting. Thanks to [Autumn Lamonte](https://gitlab.com/autumnmeowmeow)! [#1457](https://github.com/wez/wezterm/issues/1457)
* [ActivatePaneByIndex](config/lua/keyassignment/ActivatePaneByIndex.md) key assignment action. [#1517](https://github.com/wez/wezterm/issues/1517)
* Windows: wezterm may now use [win32-input-mode](https://github.com/microsoft/terminal/blob/main/doc/specs/%234999%20-%20Improved%20keyboard%20handling%20in%20Conpty.md) to send high-fidelity keyboard input to ConPTY. This means that win32 console applications, such as [FAR Manager](https://github.com/FarGroup/FarManager) that use the low level `INPUT_RECORD` API will now receive key-up events as well as events for modifier-only key presses. Use `allow_win32_input_mode=true` to enable this. [#318](https://github.com/wez/wezterm/issues/318) [#1509](https://github.com/wez/wezterm/issues/1509) [#1510](https://github.com/wez/wezterm/issues/1510)
* Windows: [default_domain](config/lua/config/default_domain.md), [wsl_domains](config/lua/config/wsl_domains.md) options and [wezterm.default_wsl_domains()](config/lua/wezterm/default_wsl_domains.md) provide more flexibility for WSL users. The effect of `add_wsl_distributions_to_launch_menu=false` was replaced by `wsl_domains={}`.
* `Symbols Nerd Font Mono` is now bundled with WezTerm and is included as a default fallback font. This means that you may use any of the glyphs available in the [Nerd Fonts](https://github.com/ryanoasis/nerd-fonts) collection with any font without patching fonts and without explicitly adding that font to your fallback list. Pomicons have an unclear license for distribution and are excluded from this bundled font, however, you may manually install the font with those icons from the Nerd Font site itself and it will take precedence over the bundled font.  This font replaces the older `PowerlineExtraSymbols` font.  [#1521](https://github.com/wez/wezterm/issues/1521).
* [wezterm.nerdfonts](config/lua/wezterm/nerdfonts.md) as a convenient way to resolve Nerd Fonts glyphs by name in your config file
* [ShowLauncherArgs](config/lua/keyassignment/ShowLauncherArgs.md) key assignment to show the launcher scoped to certain items, or to launch it directly in fuzzy matching mode
* Workspaces. Follow work in progress on [#1531](https://github.com/wez/wezterm/issues/1531) and [#1322](https://github.com/wez/wezterm/discussions/1322)! [window:active_workspace()](config/lua/window/active_workspace.md), [default_workspace](config/lua/config/default_workspace.md), [SwitchWorkspaceRelative](config/lua/keyassignment/SwitchWorkspaceRelative.md), [SwitchToWorkspace](config/lua/keyassignment/SwitchToWorkspace.md)
* `wezterm cli send-text "hello"` allows sending text, as though pasted, to a pane. See `wezterm cli send-text --help` for more information. [#888](https://github.com/wez/wezterm/issues/888)
* `local_echo_threshold_ms` option to adjust the predictive local echo timing for [SshDomain](config/lua/SshDomain.md), [TlsDomainClient](config/lua/TlsDomainClient.md) and [unix domains](multiplexing.md). Thanks to [@qperret](https://github.com/qperret)! [#1518](https://github.com/wez/wezterm/pull/1518)
* It is now possible to set `selection_fg` and `selection_bg` to be fully or partially transparent. [Read more](config/appearance.md). [#1615](https://github.com/wez/wezterm/issues/1615)
* Experimental (and incomplete!) support for Bidi/RTL can be enabled through the config. [Follow along in the tracking issue](https://github.com/wez/wezterm/issues/784)
* Primary selection is now supported on Wayland systems that implement [primary-selection-unstable-v1](https://wayland.app/protocols/primary-selection-unstable-v1) or the older Gtk primary selection protocol. Thanks to [@lunaryorn](https://github.com/lunaryorn)! [#1423](https://github.com/wez/wezterm/issues/1423)
* [pane:has_unseen_output()](config/lua/pane/has_unseen_output.md) and [PaneInformation.has_unseen_output](config/lua/PaneInformation.md) allow coloring or marking up tabs based on unseen output. [#796](https://github.com/wez/wezterm/discussions/796)
* Context menu extension for Nautilus. Thanks to [@lunaryorn](https://github.com/lunaryorn)! [#1092](https://github.com/wez/wezterm/issues/1092)
* [wezterm.enumerate_ssh_hosts()](config/lua/wezterm/enumerate_ssh_hosts.md) function that can be used to auto-generate ssh domain configuration

#### Changed

* **Key Assignments now use Physical Key locations by default!!** [Read more](config/keys.md#physical-vs-mapped-key-assignments) [#1483](https://github.com/wez/wezterm/issues/1483) [#601](https://github.com/wez/wezterm/issues/601) [#1080](https://github.com/wez/wezterm/issues/1080) [#1391](https://github.com/wez/wezterm/issues/1391)
* Key assignments now match prior to any dead-key or IME composition [#877](https://github.com/wez/wezterm/issues/877)
* Removed the `ALT-[NUMBER]` default key assignments as they are not good for non-US layouts. [#1542](https://github.com/wez/wezterm/issues/1542)
* `wezterm cli`, when run outside of a wezterm pane, now prefers to connect to the main GUI instance rather than background mux server. Use `wezterm cli --prefer-mux` to ignore the GUI instance and talk only to the mux server. See `wezterm cli --help` for additional information.
* [ScrollByPage](config/lua/keyassignment/ScrollByPage.md) now accepts fractional numbers like `0.5` to scroll by half a page at time. Thanks to [@hahuang65](https://github.com/hahuang65)! [#1534](https://github.com/wez/wezterm/pull/1534)
* [use_ime](config/lua/config/use_ime.md) now defaults to `true` on all platforms; previously it was not enabled by default on macOS.
* [canonicalize_pasted_newlines](config/lua/config/canonicalize_pasted_newlines.md) default has changed to be more compatible for `nano` users, and now provides more control over the text format that is pasted. [#1575](https://github.com/wez/wezterm/issues/1575)
* Blinking text is now eased rather than binary-blinked. See [text_blink_ease_in](config/lua/config/text_blink_ease_in.md) and [text_blink_ease_out](config/lua/config/text_blink_ease_out.md), [text_blink_rapid_ease_in](config/lua/config/text_blink_rapid_ease_in.md) and [text_blink_rapid_ease_out](config/lua/config/text_blink_rapid_ease_out.md) for more information.
* Blinking text cursor is now eased rather than binary-blinked. See [cursor_blink_ease_in](config/lua/config/cursor_blink_ease_in.md) and [cursor_blink_ease_out](config/lua/config/cursor_blink_ease_out.md).

#### Updated and Improved

* IME and dead key composition state now shows inline in the terminal using the terminal font (All platforms, except Wayland where we only support dead key composition)
* macOS: `use_ime=true` no longer prevents key repeat from working with some keys [#1131](https://github.com/wez/wezterm/issues/1131)
* Bundled harfbuzz to 4.0.1

#### Fixed

* Regression that broke fontconfig aliases such as `"monospace"` [#1250](https://github.com/wez/wezterm/pull/1250)
* Windows/X11/Wayland: CTRL+C in non-latin keyboard layouts wouldn't send CTRL+C [#678](https://github.com/wez/wezterm/issues/678)
* The new tab button in the fancy tab didn't respect `new_tab_hover` colors [#1498](https://github.com/wez/wezterm/issues/1498)
* Font baseline alignment when mixing symbols/emoji with the main text [#1499](https://github.com/wez/wezterm/issues/1499)
* Glitchy window resize [#1491](https://github.com/wez/wezterm/issues/1491)
* Ligatured glyphs no longer turn partially black when cursoring through them [#478](https://github.com/wez/wezterm/issues/478)
* Kitty Image Protocol: didn't respect `c` and `r` parameters to scale images
* Cursor location on the primary screen wasn't updated correctly if the window was resized while the alternate screen was active [#1512](https://github.com/wez/wezterm/issues/1512)
* Windows: latency issue with AltSnap and other window-managery things [#1013](https://github.com/wez/wezterm/issues/1013) [#1398](https://github.com/wez/wezterm/issues/1398) [#1075](https://github.com/wez/wezterm/issues/1075) [#1099](https://github.com/wez/wezterm/issues/1099)
* Multiplexer sessions now propagate user vars [#1528](https://github.com/wez/wezterm/issues/1528)
* Config reloads on the multiplexer server didn't cause the palette to update on the client [#1526](https://github.com/wez/wezterm/issues/1528)
* [ScrollToPrompt](config/lua/keyassignment/ScrollToPrompt.md) could get confused when there were multiple prompts on the same line [#1121](https://github.com/wez/wezterm/issues/1121)
* Hangul text in NFD was not always correctly composed when shaping fonts. [#1573](https://github.com/wez/wezterm/issues/1573)
* Avoid OOM when processing sixels with huge repeat counts [#1610](https://github.com/wez/wezterm/issues/1610)
* Set the sticky bit on socket and pid files created in `XDG_RUNTIME_DIR` to avoid removal by tmpwatch [#1601](https://github.com/wez/wezterm/issues/1601)
* Shaping combining sequences like `e U+20d7` could "lose" the vector symbol if the font produced an entry with no `x_advance`. [#1617](https://github.com/wez/wezterm/issues/1617)
* Setting the cursor color via escape sequences now take precedence over `force_reverse_video_cursor`. [#1625](https://github.com/wez/wezterm/issues/1625)
* Fixed Detection of DECSDM support via DECRQM/DECRPM, Correct sixel image placement when DECSDM is set and VT340 default sixel colors. Thanks to [Autumn](https://github.com/autumnmeowmeow)! [#1577](https://github.com/wez/wezterm/pull/1577)
* Fixed missing whitespace from intermediate lines when copying a wrapped logical line [#1635](https://github.com/wez/wezterm/issues/1635)
* Unable to match `Iosevka Term` when multiple iosevka ttc files were installed on macOS [#1630](https://github.com/wez/wezterm/issues/1630)
* Incorrect umask for panes spawned via the multiplexer server [#1633](https://github.com/wez/wezterm/issues/1633)
* Fall back from `top_left_arrow` to `left_ptr` when loading XCursor themes [#1655](https://github.com/wez/wezterm/issues/1655)
* Fixed lingering hover state in titlebar when the mouse pointer left the window. Thanks to [@davidrios](https://github.com/davidrios)! [#1434](https://github.com/wez/wezterm/issues/1434)
* We now respect the difference between `Italic` and `Oblique` font styles when matching fonts. You may explicitly specify `style="Oblique"` rather than using `italic=true` for fonts that offer both italic and oblique variants. [#1646](https://github.com/wez/wezterm/issues/1646)
* Hang when clicking a URL would launch the browser for the first time on unix systems [#1721](https://github.com/wez/wezterm/issues/1721)
* Wayland input handling gets broken after suspend/resume. Thanks to [@LawnGnome](https://github.com/LawnGnome)! [#1497](https://github.com/wez/wezterm/issues/1497)

### 20220101-133340-7edc5b5a

#### New

* Fancy Tab Bars are now the default. The default tab bar colors have changed to accommodate the new more native look.  You can turn them off by setting [use_fancy_tab_bar = false](config/lua/config/use_fancy_tab_bar.md).
* Support for the [Kitty Image Protocol](https://sw.kovidgoyal.net/kitty/graphics-protocol/) is now enabled by default.  Most of the protocol is supported; animation support is not yet implemented. Try the amazing [notcurses](https://notcurses.com/) if you want to see what modern terminal graphics can do! [#986](https://github.com/wez/wezterm/issues/986)
* unix domains now support an optional `proxy_command` to use in place of a direct unix socket connection. [Read more about multiplexing unix domains](multiplexing.html#unix-domains)
* [ScrollToTop](config/lua/keyassignment/ScrollToTop.md) and [ScrollToBottom](config/lua/keyassignment/ScrollToBottom.md) key assignments [#1360](https://github.com/wez/wezterm/issues/1360)
* [SSH Domains](config/lua/SshDomain.md) now support specifying `ssh_config` overrides. [#1149](https://github.com/wez/wezterm/issues/1149)
* [default_gui_startup_args](config/lua/config/default_gui_startup_args.md) allows defaulting to starting the ssh client (for example). [#1030](https://github.com/wez/wezterm/issues/1030)
* [mux-is-process-stateful](config/lua/mux-events/mux-is-process-stateful.md) event for finer control over prompting when closing panes. [#1412](https://github.com/wez/wezterm/issues/1412)
* [harfbuzz_features](config/font-shaping.md), [freetype_load_target](config/lua/config/freetype_load_target.md), [freetype_render_target](config/lua/config/freetype_render_target.md) and [freetype_load_flags](config/lua/config/freetype_load_flags.md) can now be overridden on a per-font basis as described in [wezterm.font](config/lua/wezterm/font.md) and [wezterm.font_with_fallback](config/lua/wezterm/font_with_fallback.md).
* [ActivateTabRelativeNoWrap](config/lua/keyassignment/ActivateTabRelativeNoWrap.md) key assignment [#1414](https://github.com/wez/wezterm/issues/1414)
* [QuickSelectArgs](config/lua/keyassignment/QuickSelectArgs.md) key assignment [#846](https://github.com/wez/wezterm/issues/846) [#1362](https://github.com/wez/wezterm/issues/1362)
* [wezterm.open_wth](config/lua/wezterm/open_with.md) function for opening URLs/documents with the default or a specific application [#1362](https://github.com/wez/wezterm/issues/1362)
* [pane:get_foreground_process_name()](config/lua/pane/get_foreground_process_name.md) method, [PaneInformation](config/lua/PaneInformation.md) now has `foreground_process_name` and `current_working_dir` fields, and [pane:get_current_working_dir](config/lua/pane/get_current_working_dir.md) is now supported on Windows for local processes, even without using OSC 7. [#1421](https://github.com/wez/wezterm/discussions/1421) [#915](https://github.com/wez/wezterm/issues/915) [#876](https://github.com/wez/wezterm/issues/876)
* [ActivatePaneDirection](config/lua/keyassignment/ActivatePaneDirection.md) now also supports `"Next"` and `"Prev"` to cycle through panes [#976](https://github.com/wez/wezterm/issues/976)
* [pane:get_logical_lines_as_text](config/lua/pane/get_logical_lines_as_text.md) to retrieve unwrapped logical lines from a pane [#1468](https://github.com/wez/wezterm/issues/1468)
* [wezterm.get_builtin_color_schemes()](config/lua/wezterm/get_builtin_color_schemes.md) function to eg: pick a random scheme per window, or otherwise reason about schemes. See [the docs](config/lua/wezterm/get_builtin_color_schemes.md) for examples!
* Added color schemes: [Alabaster](colorschemes/a/index.md#alabaster), [CGA](colorschemes/c/index.md#cga), [MaterialDesignColors](colorschemes/m/index.md#materialdesigncolors), [darkermatrix](colorschemes/d/index.md#darkermatrix), [nord-light](colorschemes/n/index.md#nord-light)

#### Changed

* quickselect: we now de-duplicate labels for results with the same textual content. [#1271](https://github.com/wez/wezterm/issues/1271)
* The default `CompleteSelectionOrOpenLinkAtMouseCursor` left button release assignment now also accepts SHIFT being held in order to make SHIFT-click `ExtendSelectionToMouseCursor` feel more ergonomic if the mouse button is released before the SHIFT key. [#1204](https://github.com/wez/wezterm/issues/1204)
* Unicode BIDI and other zero-width graphemes are now filtered out from the terminal model. It's not ideal in the sense that that information is essentially lost when copying to the clipboard, but it makes the presentation correct. [#1422](https://github.com/wez/wezterm/issues/1422)
* [use_ime](config/lua/config/use_ime.md) now defaults to `true` on X11 systems

#### Updated and Improved

* Bundled harfbuzz to 3.2.0
* Bundled freetype to 2.11.1
* Bundled NotoColorEmoji to 2.034 (with Unicode 14 support) Thanks to [@4cm4k1](https://github.com/4cm4k1)! [#1440](https://github.com/wez/wezterm/pull/1440)
* macos: removing the titlebar from `window_decorations` now preserves rounded window corners [#1034](https://github.com/wez/wezterm/issues/1034)
* Colors can now be specified in the HSL colorspace using syntax like `"hsl:235 100 50"` [#1436](https://github.com/wez/wezterm/issues/1436)
* Line/Bar cursors in [force_reverse_video_cursor](config/lua/config/force_reverse_video_cursor.md) mode now use the text foreground color rather than the cursor border color. [#1076](https://github.com/wez/wezterm/issues/1076)
* Improved logo appearance. Thanks to [@ghifarit53](https://github.com/ghifarit53)! [#1454](https://github.com/wez/wezterm/pull/1454)
* You can now pass [SendKey](config/lua/keyassignment/SendKey.md) to [wezterm.action](config/lua/wezterm/action.md) and make your `keys` config look more consistent
* Mouse wheel events are now routed to the hovered pane, rather than sent to the focused pane [#798](https://github.com/wez/wezterm/issues/798)

#### Fixed

* DECSTR (terminal soft reset) now turns off DECLRMM (left and right margin mode). Thanks to [@ninjalj](https://github.com/ninjalj)! [#1376](https://github.com/wez/wezterm/pull/1376)
* Improved conformance of CUP, HVP, SLRM, STBM escape sequences by support empty first parameter. Thanks to [@ninjalj](https://github.com/ninjalj)! [#1377](https://github.com/wez/wezterm/pull/1377)
* tab bar didn't correctly handle double-wide cells and could truncate at edges when using `format-tab-title` [#1371](https://github.com/wez/wezterm/issues/1371)
* `wezterm cli --no-auto-start` was not respected
* Pixel geometry configured on the PTY in new windows could be incorrect on HiDPI displays until the window was resized [#1387](https://github.com/wez/wezterm/issues/1387)
* Image attachment geometry for imgcat and sixels could stretch the image across the rounded up number of cells that contained the image. [#1300](https://github.com/wez/wezterm/issues/1300), [#1270](https://github.com/wez/wezterm/issues/1270)
* Closing a split pane created inside a `wezterm ssh` session wouldn't actually close the pane [#1197](https://github.com/wez/wezterm/issues/1197)
* Clicking when unfocused could lead to unwanted text selection [#1140](https://github.com/wez/wezterm/issues/1140) [#1310](https://github.com/wez/wezterm/issues/1310)
* Changing font scaling on Windows no longer causes the initial terminal rows/cols to be under-sized [#1381](https://github.com/wez/wezterm/issues/1381)
* New version update notifications are now more coordinated between multiple wezterm GUI instances, and update related configuration now respects configuration reloading. [#1402](https://github.com/wez/wezterm/issues/1402)
* [TLS domains](multiplexing.md) bootstrapping via SSH now use the `libssh` backend by default and work more reliably on Windows
* Closing a window will no longer recursively terminate contained multiplexer client panes; the window will instead be restored when you next connect to that multiplexer server. Killing/closing individual tabs/panes *will* kill the panes; this change only affects closing the window. [#848](https://github.com/wez/wezterm/issues/848) [#917](https://github.com/wez/wezterm/issues/917) [#1224](https://github.com/wez/wezterm/issues/1224)
* Colors were too intense due to over gamma correction [#1025](https://github.com/wez/wezterm/issues/1025)
* Mesa and EGL colors were too dim due to under gamma correction [#1373](https://github.com/wez/wezterm/issues/1373)
* `wezterm ssh` no longer tries to use `default_prog` or `default_cwd` when spawning additional panes on the remote host [#1456](https://github.com/wez/wezterm/issues/1456)
* Launcher menu WSL items now launch correctly on non-US versions of Windows [#1462](https://github.com/wez/wezterm/issues/1462)
* Korean text in NFD form is now correctly sized and rendered [#1474](https://github.com/wez/wezterm/issues/1474)
* macOS: `use_ime=true` conflicted with `LEADER` key assignments [#1409](https://github.com/wez/wezterm/issues/1409)
* macOS: certain keys (eg: `F8` and `F9`) did nothing when `use_ime=true`. [#975](https://github.com/wez/wezterm/issues/975)
* Splitting a tab would cause the window to lose its transparency [#1459](https://github.com/wez/wezterm/issues/1459)

### 20211205-192649-672c1cc1

#### Fixed

* Windows: `wezterm <something>` would fail silently when spawning `wezterm-gui` under the covers. Regression introduced by [#1278](https://github.com/wez/wezterm/issues/1278). Workaround is to directly spawn `wezterm-gui`.
* Windows: the PTY handles were ignored in favor of the redirected stdio handles of the parent of the wezterm mux process [#1358](https://github.com/wez/wezterm/issues/1358)
* Windows: Failure to spawn `wezterm` when launching an ssh mux domain session no longer waits forever
* "Update available" message kept showing even though already running the latest version [#1365](https://github.com/wez/wezterm/issues/1365)

### 20211204-082213-a66c61ee9

#### New

* X11 now supports IME. It currently defaults to disabled, but you can set `use_ime = true` in your config to enable it (you need to restart wezterm for this to take effect). Many thanks to [@H-M-H](https://github.com/H-M-H) for bringing xcb-imdkit to Rust and implementing this in wezterm! [#250](https://github.com/wez/wezterm/issues/250) [#1043](https://github.com/wez/wezterm/pull/1043)
* it is now possible to define colors in the range 16-255 in `colors` and color scheme definitions. Thanks to [@potamides](https://github.com/potamides)! [#841](https://github.com/wez/wezterm/issues/841) [#1056](https://github.com/wez/wezterm/pull/1056)
* Added [SendKey](config/lua/keyassignment/SendKey.md) key assignment action that makes it more convenient to rebind the key input that is sent to a pane.
* Added [Multiple](config/lua/keyassignment/Multiple.md) key assignment action for combining multuple actions in a single press.
* Added [use_resize_increments](config/lua/config/use_resize_increments.md) option to tell X11, Wayland, macOS window resizing to prefers to step in increments of the cell size
* [visual_bell](config/lua/config/visual_bell.md) and [audible_bell](config/lua/config/audible_bell.md) configuration options, as well as a [bell](config/lua/window-events/bell.md) event allows you to trigger lua code when the bell is rung. [#3](https://github.com/wez/wezterm/issues/3)
* [wezterm.action_callback](config/lua/wezterm/action_callback.md) function to make it easier to use custom events. Thanks to [@bew](https://github.com/bew)! [#1151](https://github.com/wez/wezterm/pull/1151)
* `wezterm connect` now also supports the `--class` parameter to override the window class
* [window_padding](config/lua/config/window_padding.md) now accepts values such as `"1cell"` or `"30%"` to compute values based on font or window metrics.
* BSDish systems now support [toast notifications](https://github.com/wez/wezterm/issues/489)
* [wezterm.background_child_process](config/lua/wezterm/background_child_process.md) function to spawn a process without waiting.
* [mux_env_remove](config/lua/config/mux_env_remove.md) setting to control which environment variables should be cleared prior to spawning processes in the multiplexer server [#1225](https://github.com/wez/wezterm/issues/1225)
* [canonicalize_pasted_newlines](config/lua/config/canonicalize_pasted_newlines.md) option to help Windows users manage newlines in pastes [#1213](https://github.com/wez/wezterm/issues/1213)
* SSH client now uses `libssh` by default. [ssh_backend](config/lua/config/ssh_backend.md) can be used to change this.
* [unzoom_on_switch_pane](config/lua/config/unzoom_on_switch_pane.md) option. Thanks to [@yyogo](https://github.com/yyogo) [#1301](https://github.com/wez/wezterm/issues/1301)
* [unicode_version](config/lua/config/unicode_version.md) option and corresponding OSC escape sequences that affects how the width of certain unicode sequences are interpreted.
* macOS: binaries produced by wezterm's CI are now codesigned, which resolves some spurious permission dialogs that affected some users [#482](https://github.com/wez/wezterm/issues/482)

#### Changed

* new default key assignments: CTRL+PageUp and CTRL+Tab activate next tab, CTRL+PageDown and CTRL+SHIFT+Tab activate previous tab. ALT+{1..8} directly select the first through 8th tabs. Thanks to [@friederbluemle](https://github.com/friederbluemle)! [#1132](https://github.com/wez/wezterm/pull/1132)
* X11: we now allow matching visuals with >= 8 bits per rgb value. Previously, we only matched exactly 8 bits. This improve compatibility with systems that have the COMPOSITE extension disabled. Thanks to [@shizeeg](https://github.com/shizeeg)! [#1083](https://github.com/wez/wezterm/pull/1083)
* The incomplete `Allsorts` shaper was removed.
* Windows: `wezterm-gui.exe` now only grabs the parent process' console handle when spawned from `wezterm.exe`, which prevents some frustrating interactions when launching `wezterm-gui.exe` via `start` in cmd/powershell. [#1278](https://github.com/wez/wezterm/issues/1278)
* AppImage: take care to remove APPIMAGE related environment when spawning child processes. Thanks to [@srevinsaju](https://github.com/srevinsaju)! [#1338](https://github.com/wez/wezterm/pull/1338)

#### Updated and Improved

* bundled harfbuzz updated to version 3.0.0, bundled freetype updated to 2.11
* window close confirmations now accept both uppercase and lowercase Y/N key presses. Thanks to [@SpyrosRoum](https://github.com/SpyrosRoum)! [#1119](https://github.com/wez/wezterm/pull/1119)
* multi-click-streaks are now interrupted by the cursor moving to a different cell. Thanks to [@valpackett](https://github.com/valpackett)! [#1126](https://github.com/wez/wezterm/issues/1126)
* `.deb` packages now `Provides: x-terminal-emulator`. [#1139](https://github.com/wez/wezterm/issues/1139)
* [use_cap_height_to_scale_fallback_fonts](config/lua/config/use_cap_height_to_scale_fallback_fonts.md) now computes *cap-height* based on the rasterized glyph bitmap which means that the data is accurate in more cases, including for bitmap fonts.  Scaling is now also applied across varying text styles; previously it only applied to a font within an `wezterm.font_with_fallback` font list.
* Can now match fontconfig aliases, such as `monospace`, on systems that use fontconfig. Thanks to [@valpackett](https://github.com/valpackett)! [#1149](https://github.com/wez/wezterm/issues/1149)
* Powerline semicircle glyphs now look much better. Thanks to [@bew](https://github.com/bew) and [@sdrik](https://github.com/sdrik)! [#1311](https://github.com/wez/wezterm/issues/1311)
* Splits now look better, especially when using escape sequences to change the default background color [#1256](https://github.com/wez/wezterm/issues/1256)

#### Fixed

* `wezterm cli spawn` would use the initial terminal size for a new tab, rather than using the current tab size [#920](https://github.com/wez/wezterm/issues/920)
* `text_background_opacity` opacity was not respected
* spawning commands via the mux didn't respect the `PATH` configured in `set_environment_variables`. [#1029](https://github.com/wez/wezterm/issues/1029)
* cursor could have a transparent "hole" through the window with certain cursor styles
* Consolas font + random input could cause a divide-by-zero when computing glyph metrics [#1042](https://github.com/wez/wezterm/issues/1042)
* Emoji fallback was too strict in respecting VS15/VS16 presentation selection, adjust the fallback to allow showing Emoji/Text presentation if Text/Emoji was requested but not found.
* X11: laggy input after selecting text. [#1027](https://github.com/wez/wezterm/issues/1027)
* macOS: `send_composed_key_when_left_alt_is_pressed` and `send_composed_key_when_right_alt_is_pressed` are now respected when `use_ime=true`. Thanks to [@jakelinnzy](https://github.com/jakelinnzy)! [#1096](https://github.com/wez/wezterm/pull/1096)
* X11: jittery resize with some window managers [#1051](https://github.com/wez/wezterm/issues/1051)
* X11: [window:get_appearance](config/lua/window/get_appearance.md) now actually returns Dark when the theme is dark. [#1098](https://github.com/wez/wezterm/issues/1098)
* ALT + Arrow, PageUp/PageDown, Ins, Del, Home, End incorrectly sent ESC prefixed key sequences. [#892](https://github.com/wez/wezterm/issues/892)
* Crash due to Out of Memory condition when the iTerm2 protocol was used to send excessively large PNG files [#1031](https://github.com/wez/wezterm/issues/1031)
* `DCH` (delete char) sequence would remove cells and replace them with default-blank cells instead of blank-cells-with-current-bg-color. [#789](https://github.com/wez/wezterm/issues/789)
* invisible I-beam or underline cursor when `force_reverse_video_cursor = true` [#1076](https://github.com/wez/wezterm/issues/1076)
* `SU` (scroll up) sequence would fill with default-blank cells instead of blank-cells-with-current-bg-color. [#1102](https://github.com/wez/wezterm/issues/1102)
* X11: computed but did not use the correct DPI for HiDPI screens [#947](https://github.com/wez/wezterm/issues/947)
* performance when resolving fallback fonts via fontconfig, and of coverage calculation with freetype. Thanks to [@H-M-H](https://github.com/H-M-H)!
* Wayland: incorrect initial surface size for HiDPI screens. Thanks to [@valpackett](https://github.com/valpackett)! [#1111](https://github.com/wez/wezterm/issues/1111) [#1112](https://github.com/wez/wezterm/pull/1112)
* invisible cursor in CopyMode when using kakoune [#1113](https://github.com/wez/wezterm/issues/1113)
* Wayland: `bypass_mouse_reporting_modifiers` didn't work. Thanks to [@valpackett](https://github.com/valpackett)! [#1122](https://github.com/wez/wezterm/issues/1122)
* new tabs could have the wrong number of rows and columns if a tiling WM resizes the window before OpenGL has been setup. [#1074](https://github.com/wez/wezterm/issues/1074)
* Wayland: dragging the window using the tab bar now works. Thanks to [@valpackett](https://github.com/valpackett)! [#1127](https://github.com/wez/wezterm/issues/1127)
* error matching a font when that font is in a .ttc that contains multiple font families. [#1137](https://github.com/wez/wezterm/issues/1137)
* Wayland: panic with most recent wlroots. Thanks to [@valpackett](https://github.com/valpackett)! [#1144](https://github.com/wez/wezterm/issues/1144)
* incorrect spacing for IDEOGRAPHIC SPACE. [#1161](https://github.com/wez/wezterm/issues/1161)
* italic fonts weren't always recognized as being italic, resulting in italic variants being used instead of the non-italic variants in some cases! [#1162](https://github.com/wez/wezterm/issues/1162)
* Ask freetype for cell metrics in bitmap-only fonts, rather than simply taking the bitmap width. [#1165](https://github.com/wez/wezterm/issues/1165)
* wezterm can now match bitmap fonts that are spread across multiple font files [#1189](https://github.com/wez/wezterm/issues/1189)
* ssh config parser incorrectly split `Host` patterns with commas instead of whitespace [#1196](https://github.com/wez/wezterm/issues/1196)
* search now auto-updates when the pane content changes [#1205](https://github.com/wez/wezterm/issues/1205)
* fonts with emoji presentation are shifted to better align with the primary font baseline [#1203](https://github.com/wez/wezterm/issues/1203)
* the whole tab was closed when only the zoomed pane exited. [#1235](https://github.com/wez/wezterm/issues/1235)
* multiplexer: wrong `WEZTERM_UNIX_SOCKET` environment passed to children when using unix domain sockets and `connect_automatically=true` [#1222](https://github.com/wez/wezterm/issues/1222)
* multiplexer: spawning remote tabs didn't correctly record local tab mapping, resulting in phantom additional tabs showing in the client. [#1222](https://github.com/wez/wezterm/issues/1222)
* `wezterm ls-fonts --text "✘"` didn't account for the system fallback list. [#849](https://github.com/wez/wezterm/issues/849)
* macOS: The `Menlo` font is now implicitly included in the system fallback list, as it is the only font that contains U+2718 ✘
* `wezterm cli spawn --cwd ..` and `wezterm cli split-pane --cwd ..` now resolve relative paths [#1243](https://github.com/wez/wezterm/issues/1243)
* Incorrect DECRPTUI response to DA3. Thanks to [@ninjalj](https://github.com/ninjalj)! [#1330](https://github.com/wez/wezterm/pull/1330)
* Reloading config now loads newly defined multiplexer domains, however, existing domains are not updated. [#1279](https://github.com/wez/wezterm/issues/1279)

### 20210814-124438-54e29167

* Fixed: ssh client would read `/etc/ssh/config` rather than the proper `/etc/ssh/ssh_config`
* Updated: ssh client now processes `Include` statements in ssh config
* x11: support for [VoidSymbol](config/keys.md#voidsymbol) in key assignments. Thanks to [@digitallyserviced](https://github.com/digitallyserviced)! [#759](https://github.com/wez/wezterm/pull/759)
* Fixed: UTF8-encoded-C1 control codes were not always recognized as control codes, and could result in a panic when later attempting to update the line. [#768](https://github.com/wez/wezterm/issues/768)
* Fixed: `wezterm cli split-pane` didn't use the current working dir of the source pane. [#766](https://github.com/wez/wezterm/issues/766)
* Fixed: double-click-drag selection could panic when crossing line boundaries [#762](https://github.com/wez/wezterm/issues/762)
* Fixed: wrong scaling for ligatures in Recursive Mono font [#699](https://github.com/wez/wezterm/issues/699)
* Fixed: incorrect Sixel HLS hue handling [#775](https://github.com/wez/wezterm/issues/775)
* Fixed: we now recognize the `CSI 48:2:0:214:255m` form of specifying true color text attributes [#785](https://github.com/wez/wezterm/issues/785)
* Fixed: split separators didn't respect `tab_bar_at_bottom=true` and were rendered in the wrong place [#797](https://github.com/wez/wezterm/issues/797)
* Improved: messaging around [exit_behavior](https://wezfurlong.org/wezterm/config/lua/config/exit_behavior.html)
* Fixed: errors loading custom color schemes are now logged to the error log [#794](https://github.com/wez/wezterm/issues/794)
* Fixed: OSC 7 (current working directory) now works with paths that contain spaces and other special characters. Thanks to [@Arvedui](https://github.com/Arvedui)! [#799](https://github.com/wez/wezterm/pull/799)
* Changed: the homebrew tap is now a Cask that installs to the /Applications directory on macOS. Thanks to [@laggardkernel](https://github.com/laggardkernel)!
* New: bold/dim and/or italics are now synthesized for fonts when the matching font is not actually italic or doesn't match the requested weight. [#815](https://github.com/wez/wezterm/issues/815)
* Updated: conpty.dll to v1.9.1445.0; fixes color bar artifacts when resizing window and allows win32 console applications to use mouse events
* Fixed: Windows: pane could linger after the process has died, closing only when a new pane/tab event occurs
* Fixed: Windows: first character after `wezterm ssh` keyboard authention was swallowed [#771](https://github.com/wez/wezterm/issues/771)
* Fixed: Windows: detect window resizes while authenticating for `wezterm ssh` [#696](https://github.com/wez/wezterm/issues/696)
* Fixed: OSC 52 clipboard escape didn't work in the initial pane spawned in the multiplexer server [#764](https://github.com/wez/wezterm/issues/764)
* Fixed: splitting panes in multiplexer could fail after a network reconnect [#781](https://github.com/wez/wezterm/issues/781)
* Fixed: multiplexer now propagates toast notifications and color palette to client [#489](https://github.com/wez/wezterm/issues/489) [#748](https://github.com/wez/wezterm/issues/748)
* Fixed: neovim interprets drags as double clicks [#823](https://github.com/wez/wezterm/discussions/823)
* New: `CTRL+SHIFT+L` is assigned to [ShowDebugOverlay](config/lua/keyassignment/ShowDebugOverlay.md) [#641](https://github.com/wez/wezterm/issues/641)
* Improved: antialiasing for undercurl. Thanks to [@ModProg](https://github.com/ModProg)! [#838](https://github.com/wez/wezterm/pull/838)
* Fixed: `wezterm start --cwd c:/` didn't run `default_prog`. Thanks to [@exactly-one-kas](https://github.com/exactly-one-kas)! [#851](https://github.com/wez/wezterm/pull/851)
* Improved: [skip_close_confirmation_for_processes_named](config/lua/config/skip_close_confirmation_for_processes_named.md) now includes common windows shell processes `cmd.exe`, `pwsh.exe` and `powershell.exe`. [#843](https://github.com/wez/wezterm/issues/843)
* Fixed: don't keep the window alive after running `something & disown ; exit` [#839](https://github.com/wez/wezterm/issues/839)
* Improved: we now draw sextant glyphs from the Unicode Symbols for Legacy Computing block (1FB00) when `custom_block_glyphs` is enabled.
* Changed: `COLORTERM=truecolor` is now set in the environment. [#875](https://github.com/wez/wezterm/issues/875)
* New: `wezterm cli spawn --new-window` flag for creating a new window via the CLI [#887](https://github.com/wez/wezterm/issues/887)
* Fixed: closing last pane in a tab via `CloseCurrentPane` could cause the window to close [#890](https://github.com/wez/wezterm/issues/890)
* Improved: `wezterm ls-fonts --list-system` shows all available fonts, `wezterm ls-fonts --text "hello"` explains which fonts are used for each glyph in the supplied text
* Fixed: mouse cursor is now Arrow rather than I-beam when the application in the terminal has enabled mouse reporting [#859](https://github.com/wez/wezterm/issues/859)
* Improved: DEC Special Graphics mode conformance and complete coverage of the graphics character set. Thanks to [Autumn Lamonte](https://gitlab.com/autumnmeowmeow)! [#891](https://github.com/wez/wezterm/pull/891)
* Fixed: click to focus now focuses the pane under the mouse cursor [#881](https://github.com/wez/wezterm/issues/881)
* Removed: `Parasio Dark` color scheme; it was a duplicate of the correctly named `Paraiso Dark` scheme. Thanks to [@adrian5](https://github.com/adrian5)! [#906](https://github.com/wez/wezterm/pull/906)
* Fixed: key repeat on Wayland now respects the system specified key repeat rate, and doesn't "stick". [#669](https://github.com/wez/wezterm/issues/669)
* Fixed: `force_reverse_video_cursor` wasn't correctly swapping the cursor colors in all cases. [#706](https://github.com/wez/wezterm/issues/706)
* Fixed: allow multuple `IdentityFile` lines in an ssh_config block to be considered
* Improved: implement braille characters as custom glyphs, to have perfect rendering when `custom_block_glyphs` is enabled. Thanks to [@bew](http://github.com/bew)!
* Fixed: Mod3 is no longer treater as SUPER on X11 and Wayland [#933](https://github.com/wez/wezterm/issues/933)
* Fixed: paste now respects `scroll_to_bottom_on_input`. [#931](https://github.com/wez/wezterm/issues/931)
* New: [bypass_mouse_reporting_modifiers](config/lua/config/bypass_mouse_reporting_modifiers.md) to specify which modifier(s) override application mouse reporting mode.
* Fixed: focus tracking events are now also generated when switching between panes [#941](https://github.com/wez/wezterm/issues/941)
* New: [window_frame](config/lua/config/window_frame.md) option to configure Wayland window decorations [#761](https://github.com/wez/wezterm/issues/761)
* New: [window:get_appearance()](config/lua/window/get_appearance.md) to determine if the window has a dark mode appearance, and adjust color scheme to match [#806](https://github.com/wez/wezterm/issues/806)
* Improved: [improve the new-tab button formatting](config/lua/config/tab_bar_style.md). Thanks to [@sdrik](https://github.com/sdrik)! [#950](https://github.com/wez/wezterm/pull/950)
* Fixed: if a line of text was exactly the width of the terminal it would get marked as wrappable even when followed by a newline, causing text to reflow incorrectly on resize. [#971](https://github.com/wez/wezterm/issues/971)
* Fixed: `wezterm ssh` could loop forever in the background if the connection drops and the window is closed. [#857](https://github.com/wez/wezterm/issues/857)
* Improved: VT102 conformance. Many thanks to [Autumn Lamonte](https://gitlab.com/autumnmeowmeow)! [#904](https://github.com/wez/wezterm/pull/904)
* New: [text_blink_rate](config/lua/config/text_blink_rate.md) and [text_blink_rate_rapid](config/lua/config/text_blink_rate_rapid.md) options to control blinking text. Thanks to [Autumn Lamonte](https://gitlab.com/autumnmeowmeow)! [#904](https://github.com/wez/wezterm/pull/904)
* New: Added support for [Synchronized Rendering](https://gist.github.com/christianparpart/d8a62cc1ab659194337d73e399004036) [#882](https://github.com/wez/wezterm/issues/882)
* New: wezterm now draws its own pixel-perfect versions of more block drawing glyphs.  See [custom_block_glyphs](config/lua/config/custom_block_glyphs.md) for more details. [#584](https://github.com/wez/wezterm/issues/584)
* Fixed: wayland: CursorNotFound error with the whiteglass theme. [#532](https://github.com/wez/wezterm/issues/532)
* Improved: Can now recover from exhausting available texture space by clearing the screen. [#879](https://github.com/wez/wezterm/issues/879)
* Updated bundled `Noto Color Emoji` font to version 2.028 featuring a [design update](https://blog.google/products/android/emoji-day-redesign-easier-sharing/). Thanks to [@4cm4k1](https://github.com/4cm4k1)! [#1003](https://github.com/wez/wezterm/pull/1003)
* Fixed: wayland: putting a window in the Sway scratchpad no longer blocks the wezterm process [#884](https://github.com/wez/wezterm/issues/884)
* Fixed: mouse reporting now correctly reports release events when multiple buttons are pressed and released at the same time. [#973](https://github.com/wez/wezterm/issues/973)
* Fixed: incorrect initial window/pty size when running with some tiling window managers. [#695](https://github.com/wez/wezterm/issues/695)
* New: CTRL-SHIFT-L shows the [debug overlay](config/lua/keyassignment/ShowDebugOverlay.md), which shows the error log and a lua repl. [#641](https://github.com/wez/wezterm/issues/641)
* Fixed: macOS: bright window padding on Intel-based macs [#653](https://github.com/wez/wezterm/issues/653), [#716](https://github.com/wez/wezterm/issues/716) and [#1000](https://github.com/wez/wezterm/issues/1000)
* Improved: wezterm now uses the Dual Source Blending feature of OpenGL to manage subpixel anti-aliasing alpha blending, resulting in improved appearance particularly when using a transparent window over the top of something with a light background. [#932](https://github.com/wez/wezterm/issues/932)
* Fixed: copying really long lines could falsely introduce line breaks on line wrap boundaries [#874](https://github.com/wez/wezterm/issues/874)
* New: [wezterm.add_to_config_reload_watch_list](config/lua/wezterm/add_to_config_reload_watch_list.md) function to aid with automatically reloading the config when you've split your config across multiple files. Thanks to [@AusCyberman](https://github.com/AusCyberman)! [#989](https://github.com/wez/wezterm/pull/989)
* Improved: wezterm now respects default emoji presentation and explicit emoji variation selectors (VS15 and VS16) so that glyphs that have both textual (usually monochrome, single cell width) and emoji (color, double width) presentations can be more faithfully rendered. [#997](https://github.com/wez/wezterm/issues/997)
* New: [window_background_gradient](config/lua/config/window_background_gradient.md) option to configure color gradients for your window background
* New: [wezterm.gradient_colors](config/lua/wezterm/gradient_colors.md) function to compute RGB values for gradients for use in your config.
* New: color schemes: [Abernathy](colorschemes/a/index.md#abernathy), [Ayu Mirage](colorschemes/a/index.md#ayu-mirage), [darkmatrix](colorschemes/d/index.md#darkmatrix), [Fairyfloss](colorschemes/f/index.md#fairyfloss), [GitHub Dark](colorschemes/g/index.md#github-dark), [HaX0R_BLUE](colorschemes/h/index.md#hax0r_blue), [HaX0R_GR33N](colorschemes/h/index.md#hax0r_gr33n), [HaX0R_R3D](colorschemes/h/index.md#hax0r_r3d), [Mariana](colorschemes/m/index.md#mariana), [matrix](colorschemes/m/index.md#matrix), [Peppermint](colorschemes/p/index.md#peppermint) and [UltraDark](colorschemes/u/index.md#ultradark)

### 20210502-154244-3f7122cb

* Fixed: red and blue subpixel channels were swapped, leading to excessively blurry text when using `freetype_load_flags="HorizontalLcd"`. [#639](https://github.com/wez/wezterm/issues/639)
* Fixed: the selection wouldn't always clear when the intersecting lines change [#644](https://github.com/wez/wezterm/issues/644)
* Fixed: vertical alignment issue with Iosevka on Windows [#661](https://github.com/wez/wezterm/issues/661)
* Fixed: support for "Variable" fonts such as Cascadia Code and Inconsolata on all platforms [#655](https://github.com/wez/wezterm/issues/655)
* New: [wezterm.font](config/lua/wezterm/font.md) and [wezterm.font_with_fallback](config/lua/wezterm/font_with_fallback.md) *attributes* parameter now allows matching more granular font weights and font stretch. e.g.: `wezterm.font('Iosevka Term', {stretch="Expanded", weight="Regular"})`, as fallback can specify weight/stretch/style for each individual font in the fallback.
* New: [freetype_render_target](config/lua/config/freetype_render_target.md) option for additional control over glyph rasterization.
* Fixed: `wezterm ssh HOST` no longer overrides the `User` config specified by `~/.ssh/config`
* Fixed: X11: detect when gnome DPI scaling changes [#667](https://github.com/wez/wezterm/issues/667)
* Fixed: potential panic when pasting large amounts of multi-byte text [#668](https://github.com/wez/wezterm/issues/668)
* Fixed: X11: non-ascii text could appear mangled in titlebars [#673](https://github.com/wez/wezterm/issues/673)
* Improved font IO performance and memory usage on all platforms
* New [window:toast_notification](config/lua/window/toast_notification.md) method for showing desktop notifications. [#619](https://github.com/wez/wezterm/issues/619)
* Fixed: half-pixel gaps in ligatured/double-wide glyphs depending on the font size [#614](https://github.com/wez/wezterm/issues/614)
* Fixed: Window could vanish if a tab closed while the rightmost tab was active(!) [#690](https://github.com/wez/wezterm/issues/690)
* Fixed: macOS: mouse cursor could get stuck in the hidden state. [#618](https://github.com/wez/wezterm/issues/618)
* Improved: [font_rules](config/lua/config/font_rules.md) behavior to always append reasonable default `font_rules` to those that you may have specified in your config.  `font_rules` now also include defaults for half-bright text styles.
* Improved: added [use_cap_height_to_scale_fallback_fonts](config/lua/config/use_cap_height_to_scale_fallback_fonts.md) option to scale secondary fonts according to relative their *cap-height* metric to improve size consistency.  This partially applies to some symbol/emoji fonts, but is dependent upon the font having reliable metrics.
* Improved: font-config queries now run much faster, resulting in snappier startup on unix systems
* Fixed: [Hide](config/lua/keyassignment/Hide.md) had no effect on macOS when the titlebar was disabled [#679](https://github.com/wez/wezterm/issues/679)
* Fixed: key and mouse assignments set via [window:set_config_overrides](config/lua/window/set_config_overrides.md) were not respected. [#656](https://github.com/wez/wezterm/issues/656)
* Fixed: potential panic when word selecting off top of viewport [#713](https://github.com/wez/wezterm/issues/713)
* Fixed: really long busy wait when displaying single logical json line of 1.5MB in length [#714](https://github.com/wez/wezterm/issues/714)
* New: [swallow_mouse_click_on_pane_focus](config/lua/config/swallow_mouse_click_on_pane_focus.md) option [#724](https://github.com/wez/wezterm/issues/724)
* New: [pane_focus_follows_mouse](config/lua/config/pane_focus_follows_mouse.md) option [#600](https://github.com/wez/wezterm/issues/600)
* Fixed: splitting a pane while a pane is in the zoomed state would swallow the new pane [#723](https://github.com/wez/wezterm/issues/723)
* Fixed: multi-cell glyphs weren't displayed in tab titles [#711](https://github.com/wez/wezterm/issues/711)
* New: [format-window-title](config/lua/window-events/format-window-title.md) hook for customizing the text in the window titlebar
* New: [format-tab-title](config/lua/window-events/format-tab-title.md) hook for customizing the text in tab titles. [#647](https://github.com/wez/wezterm/issues/647)
* Removed: active and inactive [tab_bar_style](config/lua/config/tab_bar_style.md) config values; use the new [format-tab-title](config/lua/window-events/format-tab-title.md) event instead
* New: [force_reverse_video_cursor](config/lua/config/force_reverse_video_cursor.md) setting to override the cursor color scheme settings. [#706](https://github.com/wez/wezterm/issues/706)
* Fixed: ssh config parsing now expands `~` to your home directory for appropriate options; previously only `%d` and `${HOME}` were substituted. [#729](https://github.com/wez/wezterm/issues/729)
* New: [Quick Select Mode](quickselect.md) for a tmux-fingers/tmux-thumbs style mouse-less select and copy flow [#732](https://github.com/wez/wezterm/issues/732)
* Fixed: tabs would remain hovered after moving the mouse down into the main terminal area [#591](https://github.com/wez/wezterm/issues/591)
* New: [tab_bar_at_bottom](config/lua/config/tab_bar_at_bottom.md) setting to put the tab bar at the bottom of the window [#278](https://github.com/wez/wezterm/issues/278)
* New: [wezterm.column_width](config/lua/wezterm/column_width.md) function for measuring the displayed width of a string
* New: [wezterm.pad_left](config/lua/wezterm/pad_left.md), [wwezterm.pad_right](config/lua/wezterm/pad_right.md), [wezterm.truncate_left](config/lua/wezterm/truncate_left.md) and [wezterm.truncate_right](config/lua/wezterm/truncate_right.md) function for truncating/padding a string based on its displayed width
* Updated bundled `Noto Color Emoji` font to version 2.020 with unicode 13.1 support. Thanks to [@4cm4k1](https://github.com/4cm4k1)! [#742](https://github.com/wez/wezterm/pull/742)
* Fixed: Numpad Enter reported as CTRL-C on macOS [#739](https://github.com/wez/wezterm/issues/739)
* Fixed: mouse reporting button state not cleared when focus is lost, eg: from clicking a link [#744](https://github.com/wez/wezterm/issues/744)
* Improved: better looking curly underline. Thanks to [@ModProg](https://github.com/ModProg)! [#733](https://github.com/wez/wezterm/pull/733)
* Fixed: wezterm now sets argv0 to `-$SHELL` to invoke a login shell, rather than running `$SHELL -l`. [#753](https://github.com/wez/wezterm/issues/753)
* Improved: `ssh_config` parsing now supports `Match` for `Host`, `LocalUser`.
* Improved render performance for wide windows [#740](https://github.com/wez/wezterm/issues/740)
* New color schemes: `Aurora`, `BlueDolphin`, `BlulocoDark`, `BlulocoLight`, `Doom Peacock`, `Galizur`, `Guezwhoz`, `PaleNightHC`, `Raycast_Dark`, `Raycast_Light`, `Sublette`, `iceberg-dark` and `iceberg-light`.

### 20210405-110924-a5bb5be8

* Fixed: bold text got broken as part of fixing #617 :-( [#648](https://github.com/wez/wezterm/issues/648)

### 20210404-112810-b63a949d

* Fixed: 100% CPU due to spurious resize events generated by herbstluftwm. [#557](https://github.com/wez/wezterm/issues/557)
* Fixed: improved conformance with xterm for keys like CTRL-6 and CTRL-/. [#556](https://github.com/wez/wezterm/discussions/556)
* Fixed: detection and handling of fonts such as terminus-bold.otb that contain only bitmap strikes. [#560](https://github.com/wez/wezterm/issues/560)
* Fixed: the pixel size reported by the pty to the kernel wasn't adjusted for font metrics/dpi until the config was reloaded or window resized. [#563](https://github.com/wez/wezterm/issues/563)
* Fixed: greatly reduce memory consumption when system fallback fonts are loaded [#559](https://github.com/wez/wezterm/issues/559)
* Fixed: Windows: `window_background_opacity` was only taking effect when `window_decorations="NONE"` [#553](https://github.com/wez/wezterm/issues/553)
* Fixed: an issue where wezterm could hang if the process spawned by a pane doesn't quit when asked [#558](https://github.com/wez/wezterm/issues/558)
* Fixed: panic when dismissing the tab navigator [#542](https://github.com/wez/wezterm/issues/542)
* Fixed: font fallback on macOS returns unresolvable `.AppleSymbolsFB` rather than `Apple Symbols`, leading to slowdowns when rendering symbols [#506](https://github.com/wez/wezterm/issues/506)
* Fixed: laggy repaints for large windows particularly on Windows, but applicable to all systems.  Tuned and triple-buffered vertex buffer updates. [#546](https://github.com/wez/wezterm/issues/546)
* Changed: [allow_square_glyphs_to_overflow_width](config/lua/config/allow_square_glyphs_to_overflow_width.md) now defaults to `WhenFollowedBySpace` and applies to more symbol glyphs. [#565](https://github.com/wez/wezterm/issues/565)
* Changed: macOS: `CMD-Q` is now bound by default to [QuitApplication](config/lua/keyassignment/QuitApplication.md)
* New: added [skip_close_confirmation_for_processes_named](config/lua/config/skip_close_confirmation_for_processes_named.md) option which specifies a list of processes for which it is considered safe to allow closing a pane/tab/window without a prompt. [#562](https://github.com/wez/wezterm/issues/562)
* Fixed: triggering the search overlay again while the search overlay is active no longer closes the underlying pane [#572](https://github.com/wez/wezterm/issues/572)
* Fixed: X10 mouse coordinate reporting encoding could produce invalid outputs for large windows. Capped coordinate values to the maximum value that is representable in UTF-8 encoding
* Fixed: font fallback now happens asynchronously from painting [#508](https://github.com/wez/wezterm/issues/508)
* New: added [window:get_selection_text_for_pane](config/lua/window/get_selection_text_for_pane.md) method [#575](https://github.com/wez/wezterm/issues/575)
* Fixed: implicit hyperlink rules, word and line selection now operate on logical lines which means that they deal with wrapped lines outside of the viewport. [#408](https://github.com/wez/wezterm/issues/408)
* New: `wezterm ssh` now supports reading `~/.ssh/config` and overriding options via the command line.  `IdentityFile` and `ProxyCommand` are the two main new supported options.  Read more about it in [ssh](ssh.md).
* Fixed: ssh support will now try all available identities from the SSH agent rather than just the first.
* New: splitting panes in `wezterm ssh` now works like spawning new tabs: the new program is started on the remote host with no additional authentication required.
* Fixed: Multiplexer sessions would fail to bootstrap via ssh because the bootstrap process exited too soon. [#507](https://github.com/wez/wezterm/issues/507)
* Fixed: Windows: we now compile libssh2 against openssl on all platforms to improve overall key and crypto algorithm support
* Fixed: spawning a new tab via the launcher menu failed because it used the pretty printed multiplexer domain label rather than the multiplexer domain name.
* Fixed: macOS: middle mouse button wasn't recognized. Thanks to [@guswynn](https://github.com/guswynn)! [#599](https://github.com/wez/wezterm/pull/599)
* New: added [ActivateLastTab](config/lua/keyassignment/ActivateLastTab.md) key assignment for jumping back to a previously active tab. Thanks to [@alexgartrell](https://github.com/alexgartrell) [#610](https://github.com/wez/wezterm/pull/610)
* Fixed: added missing XTSMGRAPHICS query/response for sixel support [#609](https://github.com/wez/wezterm/issues/609)
* Fixed: avoid showing an error dialog for synthesized `font_rules` when the configuration specifies a font that doesn't have bold/italic variants. [#617](https://github.com/wez/wezterm/issues/617)
* New: mouse cursor hides when keyboard input is sent to a pane, and shows again when the mouse is moved. [#618](https://github.com/wez/wezterm/issues/618)
* Fixed: macOS: CTRL-Tab key combination was not recognized. [#630](https://github.com/wez/wezterm/issues/630)
* Fixed: wezterm-mux-server will now continue running even after all tabs/panes have been closed. [#631](https://github.com/wez/wezterm/issues/631)
* Fixed: macOS: wezterm-gui could linger in the background until the mouse moves after all tabs/panes have closed
* Fixed: when using [line_height](config/lua/config/line_height.md), wezterm now vertically centers the cell rather than padding only the top [#582](https://github.com/wez/wezterm/issues/582)
* Fixed: macOS: in US layouts, `SUPER+SHIFT+[` was incorrectly recognized as `SUPER+SHIFT+{` instead of `SUPER+{` [#601](https://github.com/wez/wezterm/issues/601)
* Fixed: [wezterm.config_dir](config/lua/wezterm/config_dir.md) was returning the config file path instead of the directory!
* New: [wezterm.config_file](config/lua/wezterm/config_file.md) which returns the config file path

### 20210314-114017-04b7cedd

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
* Wayland: fixed opengl context creation issues.  Thanks to [@valpackett](https://github.com/valpackett)! [#481](https://github.com/wez/wezterm/pull/481)
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
* Improved and optimized image processing so that watching videos via [timg - Terminal Image and Video Viewer](http://timg.sh) works better [#537](https://github.com/wez/wezterm/issues/537) [#535](https://github.com/wez/wezterm/issues/535) [#534](https://github.com/wez/wezterm/issues/534)

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
* Added `harfbuzz_features` option to specify stylistic sets for fonts such as Fira Code, and to control various typographical options
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


