# PaneInformation

The `PaneInformation` struct describes a pane.  Unlike [the Pane
object](pane/index.md), `PaneInformation` is purely a snapshot of some of
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

