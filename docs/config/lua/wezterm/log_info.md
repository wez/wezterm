---
title: wezterm.log_info
tags:
 - utility
 - log
 - debug
---
# `wezterm.log_info(arg, ..)`

{{since('20210314-114017-04b7cedd')}}

This function logs the provided message string through wezterm's logging layer
at 'INFO' level, which can be displayed via [ShowDebugOverlay](../keyassignment/ShowDebugOverlay.md) action.  If you started wezterm from a terminal that text will print
to the stdout of that terminal.  If running as a daemon for the multiplexer
server then it will be logged to the daemon output path.

```lua
local wezterm = require 'wezterm'
wezterm.log_info 'Hello!'
```

{{since('20210814-124438-54e29167')}}

Now accepts multiple arguments, and those arguments can be of any type.


See also [log_error](log_error.md) and [log_warn](log_warn.md).

