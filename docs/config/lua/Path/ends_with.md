# `path:ends_with(path_or_string)`

{{since('nightly')}}

Determines whether `path` ends with `path_or_string`. If it does, it
returns `true`, and otherwise it returns `false`.

This method only considers whole path components to match.

```lua
local wezterm = require 'wezterm'
local to_path = wezterm.to_path

local txt = to_path '/file/path.txt'
assert(txt:ends_with 'txt' == false) -- use :extension() instead
assert(txt:ends_with 'path.txt' == true)
assert(txt:starts_with 'file/path.txt' == true)
```

See also [starts_with](starts_with.md).
