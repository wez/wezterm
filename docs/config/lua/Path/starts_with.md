# `path:starts_with(path_or_string)`

{{since('nightly')}}

Determines whether `path` starts with `path_or_string`. If it does, it
returns `true`, and otherwise it returns `false`.

This method only considers whole path components to match.

```lua
local wezterm = require 'wezterm'
local to_path = wezterm.to_path

local txt = to_path '/file/path.txt'
assert(txt:starts_with 'file' == false)
assert(txt:starts_with '/f' == false)
assert(txt:starts_with '/file' == true)
```

See also [ends_with](ends_with.md).
