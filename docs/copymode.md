## Copy Mode

*since: 20200607-144723-74889cd4*

Copy mode allows you to make selections using the keyboard; no need to reach
for your mouse or trackpad.  Copy mode is similar to [quick select
  mode](quickselect.md) but is geared up for describing selections based on
keyboard control, whereas quick select mode is used to quickly select and
copy commonly used patterns. The [colors](config/appearance.md#defining-your-own-colors)
of the highlighted/selected text can be configured.

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
| Cell selection | `v` |
| Line selection | `V` |
| Rectangular selection | `CTRL-v` (*since: 20220624-141144-bd1b7c5d*)|
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
| Move to other end of the selection horizontally| `O` (useful in Rectangular mode) |

### Configurable Key Assignments

*Since: 20220624-141144-bd1b7c5d*

The key assignments for copy mode are specified by the `copy_mode` [Key Table](config/key-tables.md).

You may provide your own definition of this key table if you wish to customize it.
There isn't a way to override portions of the key table, only to replace the entire table.

The default configuration is equivalent to:

```lua
local wezterm = require 'wezterm'
local act = wezterm.action

return {
  key_tables = {
    copy_mode = {
      { key = 'c', mods = 'CTRL', action = act.CopyMode 'Close' },
      { key = 'g', mods = 'CTRL', action = act.CopyMode 'Close' },
      { key = 'q', mods = 'NONE', action = act.CopyMode 'Close' },
      { key = 'Escape', mods = 'NONE', action = act.CopyMode 'Close' },

      { key = 'h', mods = 'NONE', action = act.CopyMode 'MoveLeft' },
      { key = 'j', mods = 'NONE', action = act.CopyMode 'MoveDown' },
      { key = 'k', mods = 'NONE', action = act.CopyMode 'MoveUp' },
      { key = 'l', mods = 'NONE', action = act.CopyMode 'MoveRight' },

      { key = 'LeftArrow', mods = 'NONE', action = act.CopyMode 'MoveLeft' },
      { key = 'DownArrow', mods = 'NONE', action = act.CopyMode 'MoveDown' },
      { key = 'UpArrow', mods = 'NONE', action = act.CopyMode 'MoveUp' },
      {
        key = 'RightArrow',
        mods = 'NONE',
        action = act.CopyMode 'MoveRight',
      },

      {
        key = 'RightArrow',
        mods = 'ALT',
        action = act.CopyMode 'MoveForwardWord',
      },
      {
        key = 'f',
        mods = 'ALT',
        action = act.CopyMode 'MoveForwardWord',
      },
      {
        key = 'Tab',
        mods = 'NONE',
        action = act.CopyMode 'MoveForwardWord',
      },
      {
        key = 'w',
        mods = 'NONE',
        action = act.CopyMode 'MoveForwardWord',
      },

      {
        key = 'LeftArrow',
        mods = 'ALT',
        action = act.CopyMode 'MoveBackwardWord',
      },
      {
        key = 'b',
        mods = 'ALT',
        action = act.CopyMode 'MoveBackwardWord',
      },
      {
        key = 'Tab',
        mods = 'SHIFT',
        action = act.CopyMode 'MoveBackwardWord',
      },
      {
        key = 'b',
        mods = 'NONE',
        action = act.CopyMode 'MoveBackwardWord',
      },

      {
        key = '0',
        mods = 'NONE',
        action = act.CopyMode 'MoveToStartOfLine',
      },
      {
        key = 'Enter',
        mods = 'NONE',
        action = act.CopyMode 'MoveToStartOfNextLine',
      },

      {
        key = '$',
        mods = 'NONE',
        action = act.CopyMode 'MoveToEndOfLineContent',
      },
      {
        key = '$',
        mods = 'SHIFT',
        action = act.CopyMode 'MoveToEndOfLineContent',
      },
      {
        key = '^',
        mods = 'NONE',
        action = act.CopyMode 'MoveToStartOfLineContent',
      },
      {
        key = '^',
        mods = 'SHIFT',
        action = act.CopyMode 'MoveToStartOfLineContent',
      },
      {
        key = 'm',
        mods = 'ALT',
        action = act.CopyMode 'MoveToStartOfLineContent',
      },

      {
        key = ' ',
        mods = 'NONE',
        action = act.CopyMode { SetSelectionMode = 'Cell' },
      },
      {
        key = 'v',
        mods = 'NONE',
        action = act.CopyMode { SetSelectionMode = 'Cell' },
      },
      {
        key = 'V',
        mods = 'NONE',
        action = act.CopyMode { SetSelectionMode = 'Line' },
      },
      {
        key = 'V',
        mods = 'SHIFT',
        action = act.CopyMode { SetSelectionMode = 'Line' },
      },
      {
        key = 'v',
        mods = 'CTRL',
        action = act.CopyMode { SetSelectionMode = 'Block' },
      },

      {
        key = 'G',
        mods = 'NONE',
        action = act.CopyMode 'MoveToScrollbackBottom',
      },
      {
        key = 'G',
        mods = 'SHIFT',
        action = act.CopyMode 'MoveToScrollbackBottom',
      },
      {
        key = 'g',
        mods = 'NONE',
        action = act.CopyMode 'MoveToScrollbackTop',
      },

      {
        key = 'H',
        mods = 'NONE',
        action = act.CopyMode 'MoveToViewportTop',
      },
      {
        key = 'H',
        mods = 'SHIFT',
        action = act.CopyMode 'MoveToViewportTop',
      },
      {
        key = 'M',
        mods = 'NONE',
        action = act.CopyMode 'MoveToViewportMiddle',
      },
      {
        key = 'M',
        mods = 'SHIFT',
        action = act.CopyMode 'MoveToViewportMiddle',
      },
      {
        key = 'L',
        mods = 'NONE',
        action = act.CopyMode 'MoveToViewportBottom',
      },
      {
        key = 'L',
        mods = 'SHIFT',
        action = act.CopyMode 'MoveToViewportBottom',
      },

      {
        key = 'o',
        mods = 'NONE',
        action = act.CopyMode 'MoveToSelectionOtherEnd',
      },
      {
        key = 'O',
        mods = 'NONE',
        action = act.CopyMode 'MoveToSelectionOtherEndHoriz',
      },
      {
        key = 'O',
        mods = 'SHIFT',
        action = act.CopyMode 'MoveToSelectionOtherEndHoriz',
      },

      { key = 'PageUp', mods = 'NONE', action = act.CopyMode 'PageUp' },
      { key = 'PageDown', mods = 'NONE', action = act.CopyMode 'PageDown' },

      { key = 'b', mods = 'CTRL', action = act.CopyMode 'PageUp' },
      { key = 'f', mods = 'CTRL', action = act.CopyMode 'PageDown' },
    },
  },
}
```
