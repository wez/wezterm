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

*Since: nightly builds only*

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
  return string.gsub(s, "(.*[/\\])(.*)", "%2")
end

wezterm.on("format-tab-title", function(tab, tabs, panes, config, hover, max_width)
  local pane = tab.active_pane
  local title = basename(pane.foreground_process_name) .. " " .. pane.pane_id
  local color = "navy"
  if tab.is_active then
    color = "blue"
  end
  return {
    {Background={Color=color}},
    {Text=" " .. title .. " "},
  }
end)

return {
}
```
