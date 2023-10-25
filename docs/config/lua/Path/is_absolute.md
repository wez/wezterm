# `path:is_absolute()`

{{since('nightly')}}

Returns `true` if `path` is an absolute `Path`, i.e., if it is independent
of the current directory.

```lua
local wezterm = require 'wezterm'
local to_path = wezterm.to_path

local rel = to_path 'file/path'
local abs = to_path '/file/path'
assert(not rel:is_absolute())
assert(abs:is_absolute())
```
