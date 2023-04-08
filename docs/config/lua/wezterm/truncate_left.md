---
title: wezterm.truncate_left
tags:
 - utility
 - string
---
# wezterm.truncate_left(string, max_width)

{{since('20210502-130208-bff6815d')}}

Returns a copy of `string` that is no longer than `max_width` columns
(as measured by [wezterm.column_width](column_width.md)).

Truncation occurs by removing excess characters from the left
end of the string.

For example, `wezterm.truncate_left("hello", 3)` returns `"llo"`.

See also: [wezterm.truncate_right](truncate_right.md), [wezterm.pad_right](pad_right.md).

