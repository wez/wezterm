# `pane:get_lines_as_text([nlines])`

{{since('20201031-154415-9614e117')}}

Returns the textual representation (not including color or other attributes) of
the *physical* lines of text in the viewport as a string.

A *physical* line is a possibly-wrapped line that composes a row in the terminal
display matrix.  If you'd rather operate on *logical* lines, see
[pane:get_logical_lines_as_text](get_logical_lines_as_text.md).

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
pane:get_lines_as_text(pane:get_dimensions().scrollback_rows)
```
