---
title: wezterm.home_path
tags:
 - utility
 - filesystem
---

# `wezterm.home_path`

{{since('nightly')}}

This constant is a [`Path`](../Path/index.md) object set to the home
directory of the user running `wezterm`.

```lua
local wezterm = require 'wezterm'
wezterm.log_error(string.format('Home: %s', wezterm.home_path))

local to_path = wezterm.to_path
assert(wezterm.home_path == to_path(wezterm.home_dir))
```

See also [home_dir](home_dir.md).
