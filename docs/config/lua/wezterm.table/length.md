# `wezterm.table.length(table)`

{{since('nightly')}}

This function returns the length of a Lua table (or array) passed to it.

Note: The Lua function `#` also returns the length of an array, but
`#` only works for array and not tables (with non-integer keys).

```lua
local wezterm = require 'wezterm'
local length = wezterm.table.length

local tbl1 = {
  a = 1,
  b = '2',
}
local arr1 = { 1, 'a', 2, 'abc' }

assert(2 == length(tbl1))
assert(4 == length(arr1))

assert(0 == #tbl1)
assert(4 == #arr1)
```

