---
title: wezterm.run_child_process
tags:
 - utility
 - open
 - spawn
---
# `wezterm.run_child_process(args)`

{{since('20200503-171512-b13ef15f')}}

This function accepts an argument list; it will attempt to spawn that command
and will return a tuple consisting of the boolean success of the invocation,
the stdout data and the stderr data.

```lua
local wezterm = require 'wezterm'

local success, stdout, stderr = wezterm.run_child_process { 'ls', '-l' }
```

See also [background_child_process](background_child_process.md)
