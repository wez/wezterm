# `tab:panes_with_info()`

{{since('20220807-113146-c2fee766')}}

Returns an array table containing an extended info entry for each of the panes
contained by this tab.

Each element is a lua table with the following fields:

* `index` - the topological pane index
* `is_active` - a boolean indicating whether this is the active pane within the tab
* `is_zoomed` - a boolean indicating whether this pane is zoomed
* `left` - The offset from the top left corner of the containing tab to the top left corner of this pane, in cells.
* `top` - The offset from the top left corner of the containing tab to the top left corner of this pane, in cells.
* `width` - The width of this pane in cells
* `height` - The height of this pane in cells
* `pixel_width` - The width of this pane in pixels
* `pixel_height` - The height of this pane in pixels
* `pane` - The [Pane](../pane/index.md) object
