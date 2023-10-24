# `path:set_extension(path_or_string)`

{{since('nightly')}}

Updates the extension of `path` to `path_or_string`.

If `path_or_string` is an empty string or `Path`, then it removes
the current extension.

Returns `false` and does nothing if `path:basename()` is `..`, and returns
true otherwise.

```lua
local wezterm = require 'wezterm'
local to_path = wezterm.to_path

local txt = to_path '/file/path.txt'
local b = txt:set_extension ''
assert(b == true)
assert(txt == to_path '/file/path')
txt:set_extension 'tar.gz'
assert(txt == to_path '/file/path.tar.gz')
```
