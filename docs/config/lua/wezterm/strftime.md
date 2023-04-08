---
title: wezterm.strftime
tags:
 - utility
 - time
 - string
---
# `wezterm.strftime(format)`

{{since('20210314-114017-04b7cedd')}}

Formats the current local date/time into a string using [the Rust chrono
strftime syntax](https://docs.rs/chrono/0.4.19/chrono/format/strftime/index.html).

```lua
local wezterm = require 'wezterm'

local date_and_time = wezterm.strftime '%Y-%m-%d %H:%M:%S'
wezterm.log_info(date_and_time)
```

See also [strftime_utc](strftime_utc.md) and [wezterm.time](../wezterm.time/index.md).

