# `path:pop()`

{{since('nightly')}}

Truncates `path` to `path:dirname()` unless `path` terminates in a
root or prefix, or if it is the empty `path`. In those cases `path`
isn't changed.

Returns `true` if `path` is truncated and `false` otherwise.

```lua
local wezterm = require 'wezterm'
local to_path = wezterm.to_path

local tmp = to_path '/file/path.txt'
local b = txt:pop()
assert(b == true)
assert(txt == to_path '/file')

txt:pop()
assert(txt == to_path '/')
```
