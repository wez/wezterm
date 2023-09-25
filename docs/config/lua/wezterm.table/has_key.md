# `wezterm.table.has_key(table, key)`

{{since('nightly')}}

This function accepts a Lua table (or array) `table` and a key `key`.
It returns `true` if `table` contains a key equal to `key` (with non-nill value)
and false otherwise.

```lua
local wezterm = require 'wezterm'
local has_key = wezterm.table.has_key

local tbl1 = {
  a = 1,
  b = '2',
}
local arr1 = { 'a', 'b', 'c' }

assert(has_key(tbl1, 'a'))
assert(not has_key(tbl1, 'c'))

assert(has_key(arr1, 3))
assert(not has_key(arr1, 4))
```
