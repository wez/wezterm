# `pane:get_logical_lines_as_text([nlines])`

{{since('20220101-133340-7edc5b5a')}}

Returns the textual representation (not including color or other attributes) of
the *logical* lines of text in the viewport as a string.

A *logical* line is an original input line prior to being wrapped into *physical*
lines to composes rows in the terminal display matrix.  WezTerm doesn't store
logical lines, but can recompute them from metadata stored in physical lines.
Excessively long logical lines are force-wrapped to constrain the cost of
rewrapping on resize and selection operations.

If you'd rather operate on physical lines, see
[pane:get_lines_as_text](get_lines_as_text.md).

If the optional `nlines` argument is specified then it is used to determine how
many lines of text should be retrieved.  The default (if `nlines` is not specified)
is to retrieve the number of lines in the viewport (the height of the pane).

The lines have trailing space removed from each line.  The lines will be
joined together in the returned string separated by a `\n` character.
Trailing blank lines are stripped, which may result in fewer lines being
returned than you might expect if the pane only had a couple of lines
of output.

To obtain the entire scrollback, you can do something like this:

```lua
pane:get_logical_lines_as_text(pane:get_dimensions().scrollback_rows)
```
