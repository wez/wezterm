# `wezterm.table.count(table)`

{{since('nightly')}}

This function returns the number of non-nil elements of any Lua table passed to it.

Note: The Lua function `#` also returns the length of an array, but `#` only works for array-style
tables with contiguous integer keys starting with index `1`, and not sparse arrays (with gaps in
their integer keys), or object or other style of tables with non-integer keys. `wezterm.table.count`
can instead be used for such tables.

```lua
local wezterm = require 'wezterm'
local count = wezterm.table.count

local tbl1 = {
  a = 1,
  b = '2',
}
local arr1 = { 1, 'a', 2, 'abc' }

assert(2 == count(tbl1))
assert(4 == count(arr1))

assert(0 == #tbl1)
assert(4 == #arr1)
```

