# `format-tab-title`

{{since('20210502-130208-bff6815d')}}

The `format-tab-title` event is emitted when the text for a tab title
needs to be recomputed.

This event is a bit special in that it is *synchronous* and must return as
quickly as possible in order to avoid blocking the GUI thread.

The most notable consequence of this is that some functions that are
asynchronous (such as
[wezterm.run_child_process](../wezterm/run_child_process.md)) are not possible
to call from inside the event handler and will generate a `format-tab-title:
runtime error: attempt to yield from outside a coroutine` error.

This example overrides the default tab title so that the background color
is blue for the active tab.  This is partially redundant because there is
already configuration for this in [tab_bar_style](../config/tab_bar_style.md),
but it demonstrates that it is possible to format more than just the text
shown in the tab.

```lua
-- This function returns the suggested title for a tab.
-- It prefers the title that was set via `tab:set_title()`
-- or `wezterm cli set-tab-title`, but falls back to the
-- title of the active pane in that tab.
function tab_title(tab_info)
  local title = tab_info.tab_title
  -- if the tab title is explicitly set, take that
  if title and #title > 0 then
    return title
  end
  -- Otherwise, use the title from the active pane
  -- in that tab
  return tab_info.active_pane.title
end

wezterm.on(
  'format-tab-title',
  function(tab, tabs, panes, config, hover, max_width)
    local title = tab_title(tab)
    if tab.is_active then
      return {
        { Background = { Color = 'blue' } },
        { Text = ' ' .. title .. ' ' },
      }
    end
    return title
  end
)
```

The parameters to the event are:

* `tab` - the [TabInformation](../TabInformation.md) for the active tab
* `tabs` - an array containing [TabInformation](../TabInformation.md) for each of the tabs in the window
* `panes` - an array containing [PaneInformation](../PaneInformation.md) for each of the panes in the active tab
* `config` - the effective configuration for the window
* `hover` - true if the current tab is in the hover state
* `max_width` - the maximum number of cells available to draw this tab when using the retro tab bar style.

The return value of the event can be:

* a string, holding the text to use for the tab title
* a table holding `FormatItem`s as used in the [wezterm.format](../wezterm/format.md) function.  This allows formatting style and color information for individual elements within the tab.

If the event encounters an error, or returns something that is not one of the
types mentioned above, then the default tab title text will be computed and
used instead.

When the tab bar is computed, this event is called twice for each tab;
on the first pass, `hover` will be `false` and `max_width` will be set
to [tab_max_width](../config/tab_max_width.md).  WezTerm will then compute
the tab widths that will fit in the tab bar, and then call the event again
for the set of tabs, this time with appropriate `hover` and `max_width`
values.

Only the first `format-tab-title` event will be executed; it doesn't make
sense to define multiple instances of the event with multiple
`wezterm.on("format-tab-title", ...)` calls.

A more elaborate example:

```lua
local wezterm = require 'wezterm'

-- The filled in variant of the < symbol
local SOLID_LEFT_ARROW = wezterm.nerdfonts.pl_right_hard_divider

-- The filled in variant of the > symbol
local SOLID_RIGHT_ARROW = wezterm.nerdfonts.pl_left_hard_divider

-- This function returns the suggested title for a tab.
-- It prefers the title that was set via `tab:set_title()`
-- or `wezterm cli set-tab-title`, but falls back to the
-- title of the active pane in that tab.
function tab_title(tab_info)
  local title = tab_info.tab_title
  -- if the tab title is explicitly set, take that
  if title and #title > 0 then
    return title
  end
  -- Otherwise, use the title from the active pane
  -- in that tab
  return tab_info.active_pane.title
end

wezterm.on(
  'format-tab-title',
  function(tab, tabs, panes, config, hover, max_width)
    local edge_background = '#0b0022'
    local background = '#1b1032'
    local foreground = '#808080'

    if tab.is_active then
      background = '#2b2042'
      foreground = '#c0c0c0'
    elseif hover then
      background = '#3b3052'
      foreground = '#909090'
    end

    local edge_foreground = background

    local title = tab_title(tab)

    -- ensure that the titles fit in the available space,
    -- and that we have room for the edges.
    title = wezterm.truncate_right(title, max_width - 2)

    return {
      { Background = { Color = edge_background } },
      { Foreground = { Color = edge_foreground } },
      { Text = SOLID_LEFT_ARROW },
      { Background = { Color = background } },
      { Foreground = { Color = foreground } },
      { Text = title },
      { Background = { Color = edge_background } },
      { Foreground = { Color = edge_foreground } },
      { Text = SOLID_RIGHT_ARROW },
    }
  end
)

return {}
```
