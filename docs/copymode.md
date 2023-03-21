{{since('20200607-144723-74889cd4')}}

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
of that region.  You can then use `Copy` (by default: `CTRL-SHIFT-C`) to copy
that region to the clipboard.

### Key Assignments

The default key assignments in copy mode are as follows:

| Action  |  Key Assignment |
|---------|-------------------|
| Activate copy mode | <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>X</kbd> |
| Copy and exit copy mode | <kbd>y</kbd> |
| Exit copy mode | <kbd>Esc</kbd>      |
|                | <kbd>Ctrl</kbd> + <kbd>C</kbd>   |
|                | <kbd>Ctrl</kbd> + <kbd>G</kbd>   |
|                | <kbd>q</kbd>        |
| Cell selection | <kbd>v</kbd> |
| Line selection | <kbd>Shift</kbd> + <kbd>V</kbd> |
| Rectangular selection | <kbd>Ctrl</kbd> + <kbd>V</kbd> {{since('20220624-141144-bd1b7c5d', inline=True)}}|
| Move Left      | <kbd>LeftArrow</kbd> |
|                | <kbd>h</kbd>        |
| Move Down      | <kbd>DownArrow</kbd> |
|                | <kbd>j</kbd>        |
| Move Up        | <kbd>UpArrow</kbd>  |
|                | <kbd>k</kbd>        |
| Move Right     | <kbd>RightArrow</kbd> |
|                | <kbd>l</kbd>         |
| Move forward one word | <kbd>Alt</kbd> + <kbd>RightArrow</kbd> |
|                       | <kbd>Alt</kbd> + <kbd>F</kbd>          |
|                       | <kbd>Tab</kbd>            |
|                       | <kbd>w</kbd>              |
| Move backward one word| <kbd>Alt</kbd> + <kbd>LeftArrow</kbd> |
|                       | <kbd>Alt</kbd> + <kbd>B</kbd>         |
|                       | <kbd>Shift</kbd> + <kbd>Tab</kbd>     |
|                       | <kbd>b</kbd>             |
| Move forward one word end    | <kbd>e</kbd> {{since('20230320-124340-559cb7b0', inline=True)}}|
| Move to start of this line     | <kbd>0</kbd> |
|                                | <kbd>Home</kbd> |
| Move to start of next line     | <kbd>Enter</kbd> |
| Move to end of this line       | <kbd>$</kbd> |
|                                | <kbd>End</kbd> |
| Move to start of indented line | <kbd>Alt</kbd> + <kbd>M</kbd> |
|                                | <kbd>^</kbd> |
| Move to bottom of scrollback   | <kbd>Shift</kbd> + <kbd>G</kbd> |
| Move to top of scrollback      | <kbd>g</kbd> |
| Move to top of viewport        | <kbd>Shift</kbd> + <kbd>H</kbd> |
| Move to middle of viewport     | <kbd>Shift</kbd> + <kbd>M</kbd> |
| Move to bottom of viewport     | <kbd>Shift</kbd> + <kbd>L</kbd> |
| Move up one screen             | <kbd>PageUp</kbd> |
|                                | <kbd>Ctrl</kbd> + <kbd>B</kbd> |
| Move up half a screen          | <kbd>Ctrl</kbd> + <kbd>U</kbd> {{since('20230320-124340-559cb7b0', inline=True)}}|
| Move down one screen           | <kbd>PageDown</kbd> |
|                                | <kbd>Ctrl</kbd> + <kbd>F</kbd>   |
| Move down half a screen        | <kbd>Ctrl</kbd> + <kbd>D</kbd> {{since('20230320-124340-559cb7b0', inline=True)}}|
| Move to other end of the selection| <kbd>o</kbd> |
| Move to other end of the selection horizontally| <kbd>Shift</kbd> + <kbd>O</kbd> (useful in Rectangular mode) |

### Configurable Key Assignments

{{since('20220624-141144-bd1b7c5d')}}

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

{% include "examples/default-copy-mode-key-table.markdown" %}
