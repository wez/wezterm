---
title: wezterm.config_file
tags:
 - filesystem
---

# `wezterm.config_file`

{{since('20210502-130208-bff6815d')}}

This constant is set to the path to the `wezterm.lua` that is in use.

```lua
local wezterm = require 'wezterm'
wezterm.log_info('Config file ' .. wezterm.config_file)
```



