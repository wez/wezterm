# PaneInformation

The `PaneInformation` struct describes a pane.  Unlike [the Pane
object](pane/index.md), `PaneInformation` is a snapshot of some of
the key characteristics of the pane, intended for use in synchronous, fast,
event callbacks that format GUI elements such as the window and tab title bars.

The `PaneInformation` struct contains the following fields:

* `pane_id` - the pane identifier number
* `pane_index` - the logical position of the pane within its containing layout
* `is_active` - is true if the pane is the active pane within its containing tab
* `is_zoomed` - is true if the pane is in the zoomed state
* `left` - the cell x coordinate of the left edge of the pane
* `top` - the cell y coordinate of the top edge of the pane
* `width` - the width of the pane in cells
* `height` - the height of the pane in cells
* `pixel_width` - the width of the pane in pixels
* `pixel_height` - the height of the pane in pixels
* `title` - the title of the pane, per [pane:get_title()](pane/get_title.md) at the time the pane information was captured
* `user_vars` - the user variables defined for the pane, per [pane:get_user_vars()](pane/get_user_vars.md) at the time the pane information was captured.
* `progress` - the progress state, per [pane:get_progress()](pane/get_progress.md) at the time the pane information was captured. {{since('nightly', inline=True)}}

{{since('20220101-133340-7edc5b5a')}}

Additional fields are available; note that accessing these may not be cheap to
compute and may slow down wezterm.  Unlike the fields listed above, these are
not pre-computed snapshots of information, so if you don't use them, you won't
pay the cost of computing them.

* `foreground_process_name` - the path to the executable image per [pane:get_foreground_process_name()](pane/get_foreground_process_name.md), or an empty string if unavailable.
* `current_working_dir` - the current working directory, per [pane:get_current_working_dir()](pane/get_current_working_dir.md). 

This example places the executable name in the tab titles:

```lua
local wezterm = require 'wezterm'

-- Equivalent to POSIX basename(3)
-- Given "/foo/bar" returns "bar"
-- Given "c:\\foo\\bar" returns "bar"
function basename(s)
  return string.gsub(s, '(.*[/\\])(.*)', '%2')
end

wezterm.on(
  'format-tab-title',
  function(tab, tabs, panes, config, hover, max_width)
    local pane = tab.active_pane
    local title = basename(pane.foreground_process_name)
      .. ' '
      .. pane.pane_id
    local color = 'navy'
    if tab.is_active then
      color = 'blue'
    end
    return {
      { Background = { Color = color } },
      { Text = ' ' .. title .. ' ' },
    }
  end
)

return {}
```

{{since('20220319-142410-0fcdea07')}}

The `has_unseen_output` field returns true if the there has been output
in the pane since the last time it was focused.

This example shows how to use this event to change the color of the
tab in the tab bar when there is unseen output.

```lua
local wezterm = require 'wezterm'
local config = {}

wezterm.on(
  'format-tab-title',
  function(tab, tabs, panes, config, hover, max_width)
    if tab.is_active then
      return {
        { Background = { Color = 'blue' } },
        { Text = ' ' .. tab.active_pane.title .. ' ' },
      }
    end
    local has_unseen_output = false
    for _, pane in ipairs(tab.panes) do
      if pane.has_unseen_output then
        has_unseen_output = true
        break
      end
    end
    if has_unseen_output then
      return {
        { Background = { Color = 'Orange' } },
        { Text = ' ' .. tab.active_pane.title .. ' ' },
      }
    end
    return tab.active_pane.title
  end
)

return config
```

{{since('20220624-141144-bd1b7c5d')}}

The `domain_name` field returns the name of the domain with which the pane is associated.

This example shows the domain name of the active pane appended to the tab title:

```lua
local wezterm = require 'wezterm'
local config = {}

wezterm.on('format-tab-title', function(tab)
  local pane = tab.active_pane
  local title = pane.title
  if pane.domain_name then
    title = title .. ' - (' .. pane.domain_name .. ')'
  end
  return title
end)

return config
```

{{since('20230408-112425-69ae8472')}}

The `tty_name` field returns the tty name with the same constraints as described
in [pane:get_tty_name()](pane/get_tty_name.md).
