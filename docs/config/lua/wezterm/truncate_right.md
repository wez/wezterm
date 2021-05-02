# wezterm.truncate_right(string, max_width)

*Since: nightly builds only*

Returns a copy of `string` that is no longer than `max_width` columns
(as measured by [wezterm.column_width](column_width.md)).

Truncation occurs by reemoving excess characters from the right end
of the string.

For example, `wezterm.truncate_right("hello", 3)` returns `"hel"`,

See also: [wezterm.truncate_left](truncate_left.md), [wezterm.pad_left](pad_left.md).
