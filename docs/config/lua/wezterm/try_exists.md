---
title: wezterm.try_exists
tags:
 - utility
 - filesystem
---

# `wezterm.try_exists`

{{since('nightly')}}

This function accepts a path in the form of a string or a [`Path`](../Path/index.md)
object and returns `true` if the path points at an existing directory, file or
(working) symlink. Otherwise it returns `false`.
This will traverse symbolic links to query information about the destination directory
or file. In case of a broken symlink it will return `false`.

If the existence of the directory or file pointed to by `path` can neither
be confirmed or denied, then this function will error. This can happen e.g.,
if permission to access the path is denied.

```lua
local wezterm = require 'wezterm'
local home_dir = wezterm.home_dir
assert(wezterm.try_exists(home_dir))
```
