## Copy Mode

*since: 20200607-144723-74889cd4*

Copy mode allows you to make selections using the keyboard; no need to reach
for your mouse or trackpad.  Copy mode is similar to [quick select
  mode](quickselect.md) but is geared up for describing selections based on
keyboard control, whereas quick select mode is used to quickly select and
copy commonly used patterns.

The `ActivateCopyMode` key assignment is used to enter copy mode; it is
bound to `CTRL-SHIFT-X` by default.

When copy mode is activated, the title is prefixed with "Copy Mode" and
the behavior of the tab is changed; keyboard input now controls the
cursor and allows moving it through the scrollback, scrolling the viewport
as needed, in a style similar to that of the Vim editor.

Move the cursor to the start of the region you wish to select and press `v` to
toggle selection mode (it is off by default), then move the cursor to the end
of that region.  You can then use `Copy` (by default: `CTRl-SHIFT-C`) to copy
that region to the clipboard.

### Key Assignments

The key assignments in copy mode are as follows.  They are not currently
reassignable.

| Action  |  Key Assignment |
|---------|-------------------|
| Exit copy mode | `Esc`      |
|                | `CTRL-C`   |
|                | `CTRL-g`   |
|                | `q`        |
| Toggle cell selection mode | `v` |
| Rectangular selection | `CTRL-v` (*since: nightly builds only*)|
| Move Left      | `LeftArrow`|
|                | `h`        |
| Move Down      | `DownArrow`|
|                | `j`        |
| Move Up        | `UpArrow`  |
|                | `k`        |
| Move Right     | `RightArrow`|
|                | `l`         |
| Move forward one word | `Alt-RightArrow` |
|                       | `Alt-f`          |
|                       | `Tab`            |
|                       | `w`              |
| Move backward one word| `Alt-LeftArrow` |
|                       | `alt-b`         |
|                       | `Shift-Tab`     |
|                       | `b`             |
| Move to start of this line     | `0` |
| Move to start of next line     | `Enter` |
| Move to end of this line       | `$` |
| Move to start of indented line | `Alt-m` |
|                                | `^` |
| Move to bottom of scrollback   | `G` |
| Move to top of scrollback      | `g` |
| Move to top of viewport        | `H` |
| Move to middle of viewport     | `M` |
| Move to bottom of viewport     | `L` |
| Move up one screen             | `PageUp` |
|                                | `CTRL-b` |
| Move down one screen           | `PageDown` |
|                                | `CTRL-f`   |
| Move to other end of the selection| `o` |
| Move to other end of the selection horizontaly| `O` (only in Rectangular mode) |

### Configurable Key Assignments

*Since: nightly builds only*

The key assignments for copy mode are specified by the `copy_mode` [Key Table](config/key-tables.md).

You may provide your own definition of this key table if you wish to customize it.
There isn't a way to override portions of the key table, only to replace the entire table.

The default configuration is equivalent to:

```lua
local wezterm = require 'wezterm'

return {
  key_tables = {
    copy_mode = {
      {key="c", mods="CTRL", action=wezterm.action{CopyMode="Close"}},
      {key="g", mods="CTRL", action=wezterm.action{CopyMode="Close"}},
      {key="q", mods="NONE", action=wezterm.action{CopyMode="Close"}},
      {key="Escape", mods="NONE", action=wezterm.action{CopyMode="Close"}},

      {key="h", mods="NONE", action=wezterm.action{CopyMode="MoveLeft"}},
      {key="j", mods="NONE", action=wezterm.action{CopyMode="MoveDown"}},
      {key="k", mods="NONE", action=wezterm.action{CopyMode="MoveUp"}},
      {key="l", mods="NONE", action=wezterm.action{CopyMode="MoveRight"}},

      {key="LeftArrow", mods="NONE", action=wezterm.action{CopyMode="MoveLeft"}},
      {key="DownArrow", mods="NONE", action=wezterm.action{CopyMode="MoveDown"}},
      {key="UpArrow", mods="NONE", action=wezterm.action{CopyMode="MoveUp"}},
      {key="RightArrow", mods="NONE", action=wezterm.action{CopyMode="MoveRight"}},

      {key="RightArrow", mods="ALT", action=wezterm.action{CopyMode="MoveForwardWord"}},
      {key="f", mods="ALT", action=wezterm.action{CopyMode="MoveForwardWord"}},
      {key="Tab", mods="NONE", action=wezterm.action{CopyMode="MoveForwardWord"}},
      {key="w", mods="NONE", action=wezterm.action{CopyMode="MoveForwardWord"}},

      {key="LeftArrow", mods="ALT", action=wezterm.action{CopyMode="MoveBackwardWord"}},
      {key="b", mods="ALT", action=wezterm.action{CopyMode="MoveBackwardWord"}},
      {key="Tab", mods="SHIFT", action=wezterm.action{CopyMode="MoveBackwardWord"}},
      {key="b", mods="NONE", action=wezterm.action{CopyMode="MoveBackwardWord"}},

      {key="0", mods="NONE", action=wezterm.action{CopyMode="MoveToStartOfLine"}},
      {key="Enter", mods="NONE", action=wezterm.action{CopyMode="MoveToStartOfNextLine"}},
      {key="$", mods="NONE", action=wezterm.action{CopyMode="MoveToEndOfLineContent"}},
      {key="$", mods="SHIFT", action=wezterm.action{CopyMode="MoveToEndOfLineContent"}},

      {key="m", mods="ALT", action=wezterm.action{CopyMode="MoveToStartOfLineContent"}},
      {key="^", mods="NONE", action=wezterm.action{CopyMode="MoveToStartOfLineContent"}},
      {key="^", mods="SHIFT", action=wezterm.action{CopyMode="MoveToStartOfLineContent"}},

      {key=" ", mods="NONE", action=wezterm.action{CopyMode="ToggleSelectionByCell"}},
      {key="v", mods="NONE", action=wezterm.action{CopyMode="ToggleSelectionByCell"}},
      {key="v", mods="CTRL", action=wezterm.action{CopyMode={SetSelectionMode="Block"}}},

      {key="G", mods="NONE", action=wezterm.action{CopyMode="MoveToScrollbackBottom"}},
      {key="G", mods="SHIFT", action=wezterm.action{CopyMode="MoveToScrollbackBottom"}},
      {key="g", mods="NONE", action=wezterm.action{CopyMode="MoveToScrollbackTop"}},

      {key="H", mods="NONE", action=wezterm.action{CopyMode="MoveToViewportTop"}},
      {key="H", mods="SHIFT", action=wezterm.action{CopyMode="MoveToViewportTop"}},
      {key="M", mods="NONE", action=wezterm.action{CopyMode="MoveToViewportMiddle"}},
      {key="M", mods="SHIFT", action=wezterm.action{CopyMode="MoveToViewportMiddle"}},
      {key="L", mods="NONE", action=wezterm.action{CopyMode="MoveToViewportBottom"}},
      {key="L", mods="SHIFT", action=wezterm.action{CopyMode="MoveToViewportBottom"}},

      {key="o", mods="NONE", action=wezterm.action{CopyMode="MoveToSelectionOtherEnd"}},
      {key="O", mods="NONE", action=wezterm.action{CopyMode="MoveToSelectionOtherEndHoriz"}},
      {key="O", mods="SHIFT", action=wezterm.action{CopyMode="MoveToSelectionOtherEndHoriz"}},

      {key="PageUp", mods="NONE", action=wezterm.action{CopyMode="PageUp"}},
      {key="PageDown", mods="NONE", action=wezterm.action{CopyMode="PageDown"}},

      {key="b", mods="CTRL", action=wezterm.action{CopyMode="PageUp"}},
      {key="f", mods="CTRL", action=wezterm.action{CopyMode="PageDown"}},
    }
  },
}
```

