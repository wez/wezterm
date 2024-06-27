# `path:set_filename(path_or_string)`

{{since('nightly')}}

Updates the filename/basename of `path` to `path_or_string`.

If `path_or_string` is an empty string or `Path`, then it removes
the current filename. If `path:basename()` is `..` or root, this is equivalent
to `path:push(path_or_string)`.

```lua
local wezterm = require 'wezterm'
local to_path = wezterm.to_path

local txt = to_path '/file/path.txt'
txt:set_filename ''
assert(txt == to_path '/file/')
txt:set_filename 'test.tar.gz'
assert(txt == to_path '/file/test.tar.gz')
```
