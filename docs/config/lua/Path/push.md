# `path:push(path_or_string)`

{{since('nightly')}}

Extend `path` with `path_or_string`.

If `path_or_string` is absolute, then it replaces `path`.

```lua
local wezterm = require 'wezterm'
local to_path = wezterm.to_path

local txt = to_path '/file/path'
txt:push 'test.txt'
assert(txt == to_path '/file/path/test.txt')

txt:push '/file/test.txt'
assert(txt == to_path '/file/test.txt')
```

See also [join](join.md).
