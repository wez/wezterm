### Shortcut / Key Binding Assignments

The default key bindings are:

| Modifiers | Key | Action |
| --------- | --- | ------ |
| `SUPER`     | `c`   | `Copy`  |
| `SUPER`     | `v`   | `Paste`  |
| `CTRL+SHIFT`     | `c`   | `Copy`  |
| `CTRL+SHIFT`     | `v`   | `Paste`  |
| `SHIFT`     | `Insert` | `Paste` |
| `SUPER`     | `m`      | `Hide`  |
| `SUPER`     | `n`      | `SpawnWindow` |
| `CTRL+SHIFT`     | `n`      | `SpawnWindow` |
| `ALT`       | `Enter`  | `ToggleFullScreen` |
| `SUPER`     | `-`      | `DecreaseFontSize` |
| `CTRL`      | `-`      | `DecreaseFontSize` |
| `SUPER`     | `=`      | `IncreaseFontSize` |
| `CTRL`      | `=`      | `IncreaseFontSize` |
| `SUPER`     | `0`      | `ResetFontSize` |
| `CTRL`      | `0`      | `ResetFontSize` |
| `SUPER`     | `t`      | `SpawnTabInCurrentTabDomain` |
| `CTRL+SHIFT`     | `t`      | `SpawnTabInCurrentTabDomain` |
| `SUPER+SHIFT` | `T`    | `SpawnTab` |
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
| `CTRL+SHIFT`     | `w`      | `CloseCurrentTab` |
| `CTRL+SHIFT`     | `1`      | `ActivateTab(0)` |
| `CTRL+SHIFT`     | `2`      | `ActivateTab(1)` |
| `CTRL+SHIFT`     | `3`      | `ActivateTab(2)` |
| `CTRL+SHIFT`     | `4`      | `ActivateTab(3)` |
| `CTRL+SHIFT`     | `5`      | `ActivateTab(4)` |
| `CTRL+SHIFT`     | `6`      | `ActivateTab(5)` |
| `CTRL+SHIFT`     | `7`      | `ActivateTab(6)` |
| `CTRL+SHIFT`     | `8`      | `ActivateTab(7)` |
| `CTRL+SHIFT`     | `9`      | `ActivateTab(8)` |
| `SUPER+SHIFT` | `[` | `ActivateTabRelative(-1)` |
| `SUPER+SHIFT` | `]` | `ActivateTabRelative(1)` |
| `CTRL+SHIFT`     | `PAGEUP`      | `MoveTabRelative(-1)` |
| `CTRL+SHIFT`     | `PAGEDOWN`      | `MoveTabRelative(1)` |

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
| `MoveTabRelative` | Move the current tab relative to its peers.  The `arg` value specifies an offset. eg: `-1` moves the tab to the left of the current tab, while `1` moves the tab to the right. |
| `MoveTab` | Move the tab so that it has the index specified by the `arg` value. eg: `0` moves the tab to be  leftmost, while `1` moves the tab so that it is second tab from the left, and so on. |

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


