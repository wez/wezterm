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

You may provide your own definition of this key table if you wish to customize
it.

You may use
[wezterm.gui.default_key_tables](config/lua/wezterm.gui/default_key_tables.md)
to obtain the defaults and extend them. In earlier versions of wezterm there
wasn't a way to override portions of the key table, only to replace the entire
table.

The default configuration at the time that these docs were built (which
may be more recent than your version of wezterm) is shown below.

You can see the configuration in your version of wezterm by running
`wezterm show-keys --lua --key-table copy_mode`.

{{#include examples/default-copy-mode-key-table.markdown}}
