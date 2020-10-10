# `pane:get_dimensions()`

*Since: nightly builds only*

Returns a lua representation of the `RenderableDimensions` struct
that identifies the dimensions and position of the viewport as
well as the scrollback for the pane.

It has the following fields:

 * `cols` the number of columns
 * `viewport_rows` the number of vertical cells in the visible portion
   of the window
 * `scrollback_rows` the total number of lines in the scrollback and viewport
 * `physical_top` the top of the physical non-scrollback screen expressed as
   a stable index.
 * `scrollback_top` the top of the scrollback; the earliest row remembered
   by wezterm.

