---
search:
  boost: 20
keywords: default keys key
tags:
 - keys
---

The default key assignments are shown in the table below.

You may also use `wezterm show-keys --lua` to see the assignments
in a form that you can copy and paste into your own configuration.

| Modifiers | Key | Action |
| --------- | --- | ------ |
| `SUPER`     | `c`   | `CopyTo="Clipboard"`  |
| `SUPER`     | `v`   | `PasteFrom="Clipboard"`  |
| `CTRL+SHIFT`     | `c`   | `CopyTo="Clipboard"`  |
| `CTRL+SHIFT`     | `v`   | `PasteFrom="Clipboard"`  |
|      | `Copy`   | `CopyTo="Clipboard"`  |
|      | `Paste`   | `PasteFrom="Clipboard"`  |
| `CTRL`     | `Insert` | `CopyTo="PrimarySelection"` {{since('20210203-095643-70a364eb', inline=True)}} |
| `SHIFT`     | `Insert` | `PasteFrom="PrimarySelection"` |
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
| `SUPER`     | `t`      | `SpawnTab="CurrentPaneDomain"` |
| `CTRL+SHIFT`     | `t`      | `SpawnTab="CurrentPaneDomain"` |
| `SUPER+SHIFT` | `T`    | `SpawnTab="DefaultDomain"` |
| `SUPER`     | `w`      | `CloseCurrentTab{confirm=true}` |
| `SUPER`     | `1`      | `ActivateTab=0` |
| `SUPER`     | `2`      | `ActivateTab=1` |
| `SUPER`     | `3`      | `ActivateTab=2` |
| `SUPER`     | `4`      | `ActivateTab=3` |
| `SUPER`     | `5`      | `ActivateTab=4` |
| `SUPER`     | `6`      | `ActivateTab=5` |
| `SUPER`     | `7`      | `ActivateTab=6` |
| `SUPER`     | `8`      | `ActivateTab=7` |
| `SUPER`     | `9`      | `ActivateTab=-1` |
| `CTRL+SHIFT`     | `w`      | `CloseCurrentTab{confirm=true}` |
| `CTRL+SHIFT`     | `1`      | `ActivateTab=0` |
| `CTRL+SHIFT`     | `2`      | `ActivateTab=1` |
| `CTRL+SHIFT`     | `3`      | `ActivateTab=2` |
| `CTRL+SHIFT`     | `4`      | `ActivateTab=3` |
| `CTRL+SHIFT`     | `5`      | `ActivateTab=4` |
| `CTRL+SHIFT`     | `6`      | `ActivateTab=5` |
| `CTRL+SHIFT`     | `7`      | `ActivateTab=6` |
| `CTRL+SHIFT`     | `8`      | `ActivateTab=7` |
| `CTRL+SHIFT`     | `9`      | `ActivateTab=-1` |
| `SUPER+SHIFT` | `[` | `ActivateTabRelative=-1` |
| `CTRL+SHIFT`     | `Tab` | `ActivateTabRelative=-1` |
| `CTRL`           | `PageUp` | `ActivateTabRelative=-1` |
| `SUPER+SHIFT` | `]` | `ActivateTabRelative=1` |
| `CTRL`           | `Tab` | `ActivateTabRelative=1` |
| `CTRL`           | `PageDown` | `ActivateTabRelative=1` |
| `CTRL+SHIFT`     | `PageUp`      | `MoveTabRelative=-1` |
| `CTRL+SHIFT`     | `PageDown`      | `MoveTabRelative=1` |
| `SHIFT`          | `PageUp`      | `ScrollByPage=-1` |
| `SHIFT`          | `PageDown`    | `ScrollByPage=1` |
| `SUPER`          | `r`    | `ReloadConfiguration` |
| `CTRL+SHIFT`     | `R`    | `ReloadConfiguration` |
| `SUPER`          | `h`    | `HideApplication` (macOS only) |
| `SUPER`          | `k`    | `ClearScrollback="ScrollbackOnly"` |
| `CTRL+SHIFT`     | `K`    | `ClearScrollback="ScrollbackOnly"` |
| `CTRL+SHIFT`     | `L`    | `ShowDebugOverlay` {{since('20210814-124438-54e29167', inline=True)}}|
| `CTRL+SHIFT`     | `P`    | `ActivateCommandPalette` {{since('20230320-124340-559cb7b0', inline=True)}}|
| `CTRL+SHIFT`     | `U`    | `CharSelect` {{since('20220903-194523-3bb1ed61', inline=True)}}|
| `SUPER`          | `f`    | `Search={CaseSensitiveString=""}` |
| `CTRL+SHIFT`     | `F`    | `Search={CaseSensitiveString=""}` |
| `CTRL+SHIFT`     | `X`    | `ActivateCopyMode` |
| `CTRL+SHIFT`     | `Space`| `QuickSelect` {{since('20210502-130208-bff6815d', inline=True)}} |
| `CTRL+SHIFT+ALT` | `"`    | `SplitVertical={domain="CurrentPaneDomain"}` |
| `CTRL+SHIFT+ALT` | `%`    | `SplitHorizontal={domain="CurrentPaneDomain"}` |
| `CTRL+SHIFT+ALT` | `LeftArrow`    | `AdjustPaneSize={"Left", 1}` |
| `CTRL+SHIFT+ALT` | `RightArrow`   | `AdjustPaneSize={"Right", 1}` |
| `CTRL+SHIFT+ALT` | `UpArrow`      | `AdjustPaneSize={"Up", 1}` |
| `CTRL+SHIFT+ALT` | `DownArrow`    | `AdjustPaneSize={"Down", 1}` |
| `CTRL+SHIFT` | `LeftArrow`    | `ActivatePaneDirection="Left"` |
| `CTRL+SHIFT` | `RightArrow`    | `ActivatePaneDirection="Right"` |
| `CTRL+SHIFT` | `UpArrow`    | `ActivatePaneDirection="Up"` |
| `CTRL+SHIFT` | `DownArrow`    | `ActivatePaneDirection="Down"` |
| `CTRL+SHIFT` | `Z`    | `TogglePaneZoomState` |

If you don't want the default assignments to be registered, you can
disable all of them with this configuration; if you chose to do this,
you must explicitly register every binding.

```lua
config.disable_default_key_bindings = true
```

!!! tip
    When using `disable_default_key_bindings`, it is recommended that you
    assign [ShowDebugOverlay](lua/keyassignment/ShowDebugOverlay.md) to
    something to aid in potential future troubleshooting.

    Likewise, you may wish to assign
    [ActivateCommandPalette](lua/keyassignment/ActivateCommandPalette.md).

