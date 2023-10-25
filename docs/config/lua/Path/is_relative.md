# `path:is_relative()`

{{since('nightly')}}

Returns `true` if `path` is a relative `Path`, i.e., it depends on the current
directory.

```lua
local wezterm = require 'wezterm'
local to_path = wezterm.to_path

local rel = to_path 'file/path'
local abs = to_path '/file/path'
assert(rel:is_relative())
assert(not abs:is_relative())
```
