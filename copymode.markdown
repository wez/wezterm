## Copy Mode

*since: 20200607-144723-74889cd4*

Copy mode allows you to make selections using the keyboard; no need to reach
for your mouse or trackpad.

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


