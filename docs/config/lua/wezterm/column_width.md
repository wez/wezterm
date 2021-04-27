# wezterm.column_width(string)

*Since: nightly builds only*

Given a string parameter, returns the number of columns that that text occupies
in the terminal, which is useful together with
[format-tab-title](../window-events/format-tab-title.md) and
[update-right-status](../window-events/update-right-status.md) to
compute/layout tabs and status information.

This is different from [string.len](https://www.lua.org/manual/5.3/manual.html#pdf-string.len)
which returns the number of bytes that comprise the string.

