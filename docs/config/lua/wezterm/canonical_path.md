---
title: wezterm.canonical_path
tags:
 - utility
 - filesystem
---
# `wezterm.canonical_path(path)`

{{since('nightly')}}

This function returns a string with the canonical form of a path if it exists.
The returned path is in absolute form with all intermediate components normalized
and symbolic links resolved.
Due to limitations in the lua bindings, all of the paths
must be able to be represented as UTF-8 or this function will generate an
error.

The function can for example be used get the correct absolute path for a path
in a different format.
```lua
local wezterm = require 'wezterm'
local canonical_path = wezterm.canonical_path

wezterm.log_error(
  wezterm.home_dir == canonical_path(wezterm.home_dir .. '/.')
)
```

Another common use case is to find the absolute path of a symlink. E.g., Dropbox is usually
symlinked to `$HOME/Dropbox` on macOS, but is located at `$HOME/Library/CloudStorage/Dropbox`.
```lua
-- macOS only:
local wezterm = require 'wezterm'
local canonical_path = wezterm.canonical_path
local home_dir = wezterm.home_dir

wezterm.log_error(
  home_dir .. '/Library/CloudStorage/Dropbox'
    == canonical_path(home_dir .. '/Dropbox')
)
```

See also [glob](glob.md).
