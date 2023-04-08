---
title: wezterm.executable_dir
tags:
 - filesystem
 - utility
---

# `wezterm.executable_dir`

This constant is set to the directory containing the `wezterm`
executable file.

```lua
local wezterm = require 'wezterm'
wezterm.log_error('Exe dir ' .. wezterm.executable_dir)
```


