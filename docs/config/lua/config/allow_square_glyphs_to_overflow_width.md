# `allow_square_glyphs_to_overflow_width = "Never"`

*Since: 20210203-095643-70a364eb*

Configures how square symbol glyph's cell is rendered:

* "WhenFollowedBySpace" - deliberately overflow the cell
  width when the next cell is a space.
* "Always" - overflow the cell regardless of the next cell
  being a space.
* "Never" (the default) - strictly respect the cell width.
