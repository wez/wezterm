---
title: wezterm.basename
tags:
 - utility
 - filesystem
---
# `wezterm.basename(path)`

{{since('nightly')}}

This function returns a string containing the basename of the given path.
The function does not check whether the given path actually exists.
Due to limitations in the lua bindings, all of the paths
must be able to be represented as UTF-8 or this function will generate an
error.

Note: This function is similar to the shell command basename, but it behaves
slightly different in some edge case. E.g. `wezterm.basename 'foo.txt/.//'`
returns `'foo.txt` since trailing `/`s are ignored and so is one `.`.
But `wezterm.basename 'foo.txt/..'` returns `'..'`. This behaviour comes
from Rust's [`std::path::PathBuf`](https://doc.rust-lang.org/nightly/std/path/struct.PathBuf.html#method.file_name).

```lua
local wezterm = require 'wezterm'
local basename = wezterm.basename

wezterm.log_info('baz.txt = ' .. basename '/foo/bar/baz.txt')
```

See also [dirname](dirname.md).
