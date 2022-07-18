# MuxTab

*Since: 20220624-141144-bd1b7c5d*

`MuxTab` represents a tab that is managed by the multiplexer.

It has the following methods:

## `tab:tab_id()`

Returns the tab id


## tab:get_title()

*Since: nightly builds only*

Returns the tab title as set by `tab:set_title()`.

## tab:set_title(TITLE)

*Since: nightly builds only*

Sets the tab title to the provided string.

```lua
tab:set_title 'my title'
```

## tab:window()

*Since: nightly builds only*

Returns the [MuxWindow](mux-window/index.md) object that contains this tab.

## tab:panes()

*Since: nightly builds only*

Returns an array table containing the set of [MuxPane](MuxPane.md) objects
contained by this tab.

## tab:panes_with_info()

*Since: nightly builds only*

Returns an array table containing an extended info entry for each of the panes
contained by this tab.

Each element is a lua table with the following fields:

* `index` - the topological pane index
* `is_active` - a boolean indicating whether this is the active pane withing the tab
* `is_zoomed` - a boolean indicating whether this pane is zoomed
* `left` - The offset from the top left corner of the containing tab to the top left corner of this pane, in cells.
* `top` - The offset from the top left corner of the containing tab to the top left corner of this pane, in cells.
* `width` - The width of this pane in cells
* `height` - The height of this pane in cells
* `pixel_width` - The width of this pane in pixels
* `pixel_height` - The height of this pane in pixels
* `pane` - The [MuxPane](MuxPane.md) object

