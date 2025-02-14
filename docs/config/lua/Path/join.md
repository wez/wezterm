# `path:join(path_or_string)`

{{since('nightly')}}

Creates a new `Path` object with `path_or_string` adjoined to `path`.

If `path_or_string` is absolute, then it replaces `path`.

```lua
local wezterm = require 'wezterm'
local to_path = wezterm.to_path

local tmp = to_path '/file/path'
local txt = tmp:join 'test.txt'
assert(tmp == to_path '/file/path')
assert(txt == to_path '/file/path/test.txt')

txt = tmp:join '/file/test.txt'
assert(txt == to_path '/file/test.txt')
```

See also [push](push.md).
