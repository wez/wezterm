# `wezterm.table.has_value(table, value)`

{{since('nightly')}}

This function accepts a Lua table (or array) `table` and a value `value`.
It returns `true` if `table` contains an entry with value equal to `value`
and false otherwise.

```lua
local wezterm = require 'wezterm'
local has_value = wezterm.table.has_value

local tbl1 = {
  a = 1,
  b = '2',
}
local arr1 = { 'a', 'b', 'c' }

assert(has_value(tbl1, 1))
assert(not has_value(tbl1, 'a'))

assert(has_value(arr1, 'a'))
assert(not has_value(arr1, '1'))
```
