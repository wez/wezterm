---
title: wezterm.running_under_wsl
tags:
 - utility
---
# `wezterm.running_under_wsl()`

This function returns a boolean indicating whether we believe that we are
running in a Windows Services for Linux (WSL) container.  In such an
environment the `wezterm.target_triple` will indicate that we are running in
Linux but there will be some slight differences in system behavior (such as
filesystem capabilities) that you may wish to probe for in the configuration.

```lua
local wezterm = require 'wezterm'
wezterm.log_error(
  'System '
    .. wezterm.target_triple
    .. ' '
    .. tostring(wezterm.running_under_wsl())
)
```


