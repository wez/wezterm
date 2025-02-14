---
title: wezterm.to_path
tags:
 - utility
 - filesystem
---

# `wezterm.to_path`

{{since('nightly')}}

This function takes a string and convert it to a [`Path`](../Path/index.md)
object.

```lua
local wezterm = require 'wezterm'
local to_path = wezterm.to_path

assert(wezterm.home_path == to_path(wezterm.home_dir))
```

