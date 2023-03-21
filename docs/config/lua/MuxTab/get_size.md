# `tab:get_size()`

{{since('20230320-124340-559cb7b0')}}

Returns the overall size of the tab, taking into account all of the contained
panes.

The return value is a lua table with the following fields:

* `rows` - the number of rows (height)
* `cols` - the number of columns (width)
* `pixel_width` - the total width, measured in pixels
* `pixel_height` - the total height, measured in pixels
* `dpi` - the resolution of the tab.

Note that `pixel_width`, `pixel_height` and `dpi` may be inaccurate when there
is no GUI client associated with the tab.


