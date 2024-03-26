# `wezterm.table.equal(table1, table2)`

{{since('nightly')}}

This function checks if two tables are equal by checking for equality of their values. The
function returns `true` if `table1` and `table2` are equal and `false` otherwise.

Note: Lua can also check equality of tables via `==`, but `==` checks if the table addresses
are equal, and thus it cannot be used to check equality of values like this function. E.g.,
`{1} == {1}` will return false, whereas `wezterm.table.equal({1}, {1})` returns true.

```lua
local wezterm = require 'wezterm'
local equal = wezterm.table.equal

local tbl1 = {
  a = 1,
  b = '2',
}
local arr1 = { 1, 'a', 2, 'abc' }

assert(not tbl1 == arr1)
assert(not equal(tbl1, arr1))

assert(equal(tbl1, { a = 1, b = '2' }))
assert(not (tbl1 == { a = 1, b = '2' }))

assert(equal(arr1, { 1, 'a', 2, 'abc' }))
assert(not (arr1 == { 1, 'a', 2, 'abc' }))
```

