# `allow_square_glyphs_to_overflow_width = "Never"`

*Since: 20210203-095643-70a364eb*

Configures how square symbol glyph's cell is rendered:

* "WhenFollowedBySpace" - deliberately overflow the cell
  width when the next cell is a space.
* "Always" - overflow the cell regardless of the next cell
  being a space.
* "Never" (the default) - strictly respect the cell width.

*Since: 20210404-112810-b63a949d*

This setting now applies to any glyph with an aspect ratio
larger than 0.9, which covers more symbol glyphs than in
earlier releases.

The default value for this setting has changed to `WhenFollowedBySpace`.
