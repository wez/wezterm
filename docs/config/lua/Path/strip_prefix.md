# `path:strip_prefix(path_or_string)`

{{since('nightly')}}

Returns a `Path` object that when joined onto `path_or_string`, yields `path`.

If `path_or_string` is not a prefix of `path`, then it returns `path`.

```lua
local wezterm = require 'wezterm'
local to_path = wezterm.to_path

local txt = to_path '/file/path.txt'
assert(txt:strip_prefix '/' == to_path 'file/path.txt')
assert(txt:strip_prefix '/file' == to_path 'path.txt')
assert(txt:strip_prefix '/file/' == to_path 'path.txt')
```
