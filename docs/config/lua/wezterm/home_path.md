---
title: wezterm.home_dir
tags:
 - utility
 - filesystem
---

# `wezterm.home_path`

This constant is a `Path` object set to the home directory of the user running `wezterm`.

```lua
local wezterm = require 'wezterm'
wezterm.log_error(string.format("Home: %s", wezterm.home_path))
```

See also (home_dir)[home_dir.md].
