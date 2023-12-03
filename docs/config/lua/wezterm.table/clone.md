# `wezterm.table.clone(table [, behavior])`

{{since('nightly')}}

This function clones the Lua table passed to it.

The function accepts an optional string of the form `'Top'` or `'Deep'`
describing its behavior. Any other string passed to the function will result
in an error. The default behavior is equavalent to passing the string `'Top'`
as the behavior.

When `clone` is run with the `'Top'` behavior, it will only go through the
top-level of the table and clone the values. In particular this means that
for any tables at the top-level, it will just clone the table address, and
thus any changes in these nested tables will affect the clone.

When `clone` is run with the `'Deep'` behavior, it will recursively go through
all nested tables and clone the non-table values. Thus any changes to the
original table won't affect the clone.


```lua
local wezterm = require 'wezterm'
local clone = wezterm.table.clone
local equal = wezterm.table.equal

local tbl = {
  a = 1,
  b = '2',
  c = {
    d = 1,
  },
}
local tbl_copy = tbl -- copy the table address
local tbl_top_clone = clone(tbl) -- same as clone(tbl1, 'Top')
local tbl_deep_clone = clone(tbl, 'Deep')

assert(tbl == tbl_copy)
assert(not (tbl == tbl_top_clone))
assert(not (tbl == tbl_deep_clone))
assert(equal(tbl, tbl_top_clone))
assert(equal(tbl, tbl_deep_clone))

tbl.a = 2
assert(not equal(tbl, tbl_top_clone))
assert(tbl_top_clone.a == 1)
assert(not equal(tbl, tbl_deep_clone))
assert(tbl_deep_clone.a == 1)
tbl.a = 1

tbl.c.d = 2
assert(equal(tbl, tbl_top_clone))
assert(tbl_deep_clone.c.d == 2)
assert(not equal(tbl, tbl_deep_clone))
assert(tbl_deep_clone.c.d == 1)
```

