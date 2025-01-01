---
title: wezterm.home_dir
tags:
 - utility
 - filesystem
---

# `wezterm.home_dir`

This constant is set to the home directory of the user running `wezterm`.

```lua
local wezterm = require 'wezterm'
wezterm.log_error('Home ' .. wezterm.home_dir)
```


