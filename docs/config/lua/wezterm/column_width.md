---
title: wezterm.column_width
tags:
 - utility
 - string
---

# wezterm.column_width(string)

{{since('20210502-130208-bff6815d')}}

Given a string parameter, returns the number of columns that that text occupies
in the terminal, which is useful together with
[format-tab-title](../window-events/format-tab-title.md) and
[update-right-status](../window-events/update-right-status.md) to
compute/layout tabs and status information.

This is different from [string.len](https://www.lua.org/manual/5.3/manual.html#pdf-string.len)
which returns the number of bytes that comprise the string.

