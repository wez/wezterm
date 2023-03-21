# `pane:get_text_from_region(start_x, start_y, end_x, end_y)`

{{since('20230320-124340-559cb7b0')}}

Returns the text from the specified region.

* `start_x` and `end_x` are the starting and ending cell column, where 0 is the
  left-most cell
* `start_y` and `end_y` are the starting and ending row, expressed as a stable
  row index.  Use [pane:get_dimensions()](get_dimensions.md) to retrieve the
  currently valid stable index values for the top of scrollback and top of
  viewport.

The text within the region is unwrapped to its logical line representation,
rather than the wrapped-to-physical-display-width.

