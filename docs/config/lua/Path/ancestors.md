# `path:ancestors()`

{{since('nightly')}}

Returns an array of Path objects going through the elements obtained
by repeatedly calling `path:dirname()` until you hit the root.

```lua
local wezterm = require 'wezterm'
local to_path = wezterm.to_path

local txt = to_path '/some/file/path.txt'
local arr = {
  to_path '/some/file/path.txt',
  to_path '/some/file',
  to_path '/some',
  to_path '/',
}
assert(arr == txt.ancestors())
```
