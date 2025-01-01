---
tags:
  - font
---
# `underline_thickness`

{{since('20221119-145034-49b9839f')}}

If specified, overrides the base thickness of underlines. The underline
thickness is also used for rendering split pane dividers and a number of other
lines in custom glyphs.

The default is to use the underline thickness metric specified by the designer
of the primary font.

This config option accepts different units that have slightly different interpretations:

* `2`, `2.0` or `"2px"` all specify a thickness of 2 pixels
* `"2pt"` specifies a thickness of 2 points, which scales according to the DPI of the window
* `"200%"` takes the font-specified `underline_thickness` and multiplies it by 2 to arrive at a thickness double the normal size
* `"0.1cell"` takes the cell height, scales it by `0.1` and uses that as the thickness


