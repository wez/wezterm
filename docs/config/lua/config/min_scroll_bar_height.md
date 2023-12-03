---
tags:
  - appearance
  - scroll_bar
---
# `min_scroll_bar_height = "0.5cell"`

{{since('20220624-141144-bd1b7c5d')}}

Controls the minimum size of the scroll bar "thumb".

The value can be a number to specify the number of pixels, or a string with a unit suffix:

* `"1px"` - the `px` suffix indicates pixels, so this represents a `1` pixel value
* `"1pt"` - the `pt` suffix indicates points.  There are `72` points in `1 inch`.  The actual size this occupies on screen depends on the dpi of the display device.
* `"1cell"` - the `cell` suffix indicates the height of the terminal cell, which in turn depends on the font size, font scaling and dpi.
* `"1%"` - the `%` suffix indicates the size of the terminal portion of the display, which is computed based on the number of rows and the size of the cell.

You may use a fractional number such as `"0.5cell"` or numbers larger than one such as `"72pt"`.
