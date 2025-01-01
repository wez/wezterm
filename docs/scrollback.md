WezTerm provides a searchable scrollback buffer with a configurable maximum
size limit that allows you to review information that doesn't fit in the
physical window size.  As content is printed to the display the display may be
scrolled up to accommodate newly added lines.  The scrolled lines are moved
into the scrollback buffer and can be reviewed by scrolling the window up or
down.

This section describes working with the scrollback and discusses some
configuration options; be sure to read the [configuration
docs](config/files.md) to learn how to change your settings!

### Controlling the scrollback size

This value serves as an upper bound on the number of lines.
The larger this value, the more memory is required to manage the tab.
If you have a lot of long lived tabs then making this value very large
may put some pressure on your system depending on the amount of RAM
you have available.

```lua
-- How many lines of scrollback you want to retain per tab
config.scrollback_lines = 3500
```

### Clearing the scrollback buffer

By default, `CTRL-SHIFT-K` and `CMD-K` will trigger the `ClearScrollback`
action and discard the contents of the scrollback buffer.  There is no way
to undo discarding the scrollback.

See the [ClearScrollback](config/lua/keyassignment/ClearScrollback.md) docs for information
on rebinding this key.

### Enable/Disable scrollbar

You can control whether WezTerm displays a scrollbar via your configuration
file:

```lua
-- Enable the scrollbar.
-- It will occupy the right window padding space.
-- If right padding is set to 0 then it will be increased
-- to a single cell width
config.enable_scroll_bar = true
```

You may [change the color of the scrollbar](config/appearance.md#defining-your-own-colors) if you wish!

### Scrolling without a scrollbar

By default, `SHIFT-PageUp` and `SHIFT-PageDown` will adjust the viewport scrollback position
by one full screen for each press.

See the [ScrollByPage](config/lua/keyassignment/ScrollByPage.md) docs for more information
on this key binding assignment.

### Searching the scrollback

By default, `CTRL-SHIFT-F` and `CMD-F` (`F` for `Find`) will activate the
search overlay in the current tab.

When the search overlay is active the behavior of wezterm changes:

* Typing (or pasting) text will populate the *search pattern* in the bar at the bottom of the screen.
* Text from the scrollback that matches the *search pattern* will be highlighted and
  the number of matches shown in the search bar.
* The bottom-most match will be selected and the viewport scrolled to show the selected
  text.
* `Enter`, `UpArrow` and `CTRL-P` will cause the selection to move to any prior matching text.
* `PageUp` will traverse to previous matches one page at a time.
* `CTRL-N` and `DownArrow` will cause the selection to move to any next matching text.
* `PageDown` will traverse to the next match one page at a time.
* `CTRL-R` will cycle through the pattern matching mode; the initial mode is case-sensitive
  text matching, the next will match ignoring case and the last will match using the
  [regular expression syntax described here](https://docs.rs/regex/1.3.9/regex/#syntax).
  The matching mode is indicated in the search bar.
* `CTRL-U` will clear the *search pattern* so you can start over.
* `CTRL-SHIFT-C` will copy the selected text to the clipboard.
* `Escape` will cancel the search overlay, leaving the currently selected text selected
  with the viewport scrolled to that location.

#### Configurable search mode key assignments

{{since('20220624-141144-bd1b7c5d')}}

The key assignments for search mode are specified by the `search_mode` [Key Table](config/key-tables.md).

You may use
[wezterm.gui.default_key_tables](config/lua/wezterm.gui/default_key_tables.md)
to obtain the defaults and extend them. In earlier versions of wezterm there
wasn't a way to override portions of the key table, only to replace the entire
table.

The default configuration at the time that these docs were built (which
may be more recent than your version of wezterm) is shown below.

You can see the configuration in your version of wezterm by running
`wezterm show-keys --lua --key-table search_mode`.

{% include "examples/default-search-mode-key-table.markdown" %}

(Those assignments reference `CopyMode` because search mode is a facet of [Copy Mode](copymode.md)).

### Configuring Saved Searches

{{since('20200607-144723-74889cd4')}}

If you find that you're often searching for the same things then you may wish to assign
a keybinding to trigger that search.

For example, if you find that you're frequently running `git log` and then reaching
for your mouse to copy and paste a relevant git commit hash then you might like
this:

```lua
config.keys = {
  -- search for things that look like git hashes
  {
    key = 'H',
    mods = 'SHIFT|CTRL',
    action = wezterm.action.Search { Regex = '[a-f0-9]{6,}' },
  },
}
```

With that in your config you can now:

* `CTRL-SHIFT-H` to highlight all the git hashes and select the closest one to the bottom
  of the screen.
* Use `ENTER`/`CTRL-N`/`CTRL-P` to cycle through the git hashes
* `CTRL-SHIFT-C` to copy
* `Escape`
* `CTRL-SHIFT-V` (or `SHIFT-Insert`) to Paste

without needing to reach for your mouse.

See [the Search action docs](config/lua/keyassignment/Search.md) for more information on
using the `Search` action.
