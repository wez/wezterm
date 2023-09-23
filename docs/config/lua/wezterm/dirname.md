---
title: wezterm.dirname
tags:
 - utility
 - filesystem
---
# `wezterm.dirname(path)`

{{since('nightly')}}

This function returns a string containing the dirname of the given path.
The function does not check whether the given path actually exists.
Due to limitations in the lua bindings, all of the paths
must be able to be represented as UTF-8 or this function will generate an
error.

Note: This function is similar to the shell command dirname, but it might
behave slightly different in some edge case.

```lua
local wezterm = require 'wezterm'
local dirname = wezterm.dirname

wezterm.log_error( '/foo/bar = ' .. dirname '/foo/bar/baz.txt' )
```

If you want only the directory name and not the full path, you can use
`basename` and `dirname` together. E.g.:
```lua
local wezterm = require 'wezterm'
local basename = wezterm.basename
local dirname = wezterm.dirname

wezterm.log_error( 'bar = ' .. basename(dirname '/foo/bar/baz.txt') )
```

See also [basename](basename.md).
