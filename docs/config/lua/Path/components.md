# `path:components()`

{{since('nightly')}}

Returns an array of Path objects going through the components of `path`.

```lua
local wezterm = require 'wezterm'
local to_path = wezterm.to_path

local txt = to_path '/some/file/path.txt'
local arr = {
  to_path '/',
  to_path 'some',
  to_path 'file',
  to_path 'path.txt',
}
assert(arr == txt.components())
```
