---
tags:
  - appearance
---
# `window_padding`

Controls the amount of padding between the window border and the
terminal cells.

Padding is measured in pixels.

If [enable_scroll_bar](enable_scroll_bar.md) is `true`, then the value you
set for `right` will control the width of the scrollbar.  If you have
enabled the scrollbar and have set `right` to `0` then the right padding
(and thus the scrollbar width) will instead match the width of a cell.

```lua
config.window_padding = {
  left = 2,
  right = 2,
  top = 0,
  bottom = 0,
}
```

{{since('20211204-082213-a66c61ee9')}}

You may now express padding using a number of different units by specifying
a string value with a unit suffix:

* `"1px"` - the `px` suffix indicates pixels, so this represents a `1` pixel value
* `"1pt"` - the `pt` suffix indicates points.  There are `72` points in `1 inch`.  The actual size this occupies on screen depends on the dpi of the display device.
* `"1cell"` - the `cell` suffix indicates the size of the terminal cell, which in turn depends on the font size, font scaling and dpi.  When used for width, the width of the cell is used.  When used for height, the height of the cell is used.
* `"1%"` - the `%` suffix indicates the size of the terminal portion of the display, which is computed based on the number of rows/columns and the size of the cell.  While it is possible to specify percentage, there are some resize scenarios where the percentage value may not be 100% stable/deterministic, as the size of the padding is used to compute the number of rows/columns.

You may use a fractional number such as `"0.5cell"` or numbers larger than one such as `"72pt"`.

The default padding is shown below.  In earlier releases, the default padding was 0 for each of the possible edges.

```lua
config.window_padding = {
  left = '1cell',
  right = '1cell',
  top = '0.5cell',
  bottom = '0.5cell',
}
```

