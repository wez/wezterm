---
tags:
  - appearance
  - text_cursor
---
# `cursor_thickness`

{{since('20221119-145034-49b9839f')}}

If specified, overrides the base thickness of the lines used to
render the textual cursor glyph.

The default is to use the [underline_thickness](underline_thickness.md).

This config option accepts different units that have slightly different interpretations:

* `2`, `2.0` or `"2px"` all specify a thickness of 2 pixels
* `"2pt"` specifies a thickness of 2 points, which scales according to the DPI of the window
* `"200%"` takes the `underline_thickness` and multiplies it by 2 to arrive at a thickness double the normal size
* `"0.1cell"` takes the cell height, scales it by `0.1` and uses that as the thickness

