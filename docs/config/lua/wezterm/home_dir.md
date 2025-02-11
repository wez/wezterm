---
title: wezterm.home_dir
tags:
 - utility
 - filesystem
---

# `wezterm.home_dir`

This constant is a string set to the home directory of the user running `wezterm`.

```lua
local wezterm = require 'wezterm'
wezterm.log_error('Home ' .. wezterm.home_dir)
```

{{since('nightly')}}

```lua
local wezterm = require 'wezterm'
assert(wezterm.home_dir == tostring(wezterm.home_path))
```

See also [home_path](home_path.md).
