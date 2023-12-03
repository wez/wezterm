# `wezterm.table.flatten(array_of_arrays [, behavior])`

{{since('nightly')}}

This function flattens Lua arrays passed to it in the form of an array.
I.e., to flatten the Lua arrays `arr1` and `arr2` into one array,
we can pass them to the function as `{ arr1, arr2 }`. (See below.)

The function accepts an optional string of the form `'Top'` or `'Deep'`
describing its behavior. Any other string passed to the function will result
in an error. The default behavior is equavalent to passing the string `'Top'`
as the behavior.

When `flatten` is run with the `'Top'` behavior, it will only go through the
top-level of the arrays and flatten the values into a new array. In particular
this means that for any table at the top-level in one of the arrays, it will
not try to flatten the table and instead will just add the table to the top-level
of the flattened array.

When `flatten` is run with the `'Deep'` behavior, it will recursively go through
all nested tables and treat them like array-like tables that it then flattens.

```lua
local wezterm = require 'wezterm'
local flatten = wezterm.table.flatten
local equal = wezterm.table.equal

local arr1 = { { 1, 2 }, 3 }
local arr2 = { 'a', { 'b', { 'c' } } }
local arr3 = { 1, { a = 1, 2 }, { b = 2 } }

assert(equal(flatten { arr1, arr2 }, { { 1, 2 }, 3, 'a', { 'b', { 'c' } } }))
assert(equal(flatten({ arr1, arr2 }, 'Deep'), { 1, 2, 3, 'a', 'b', 'c' }))

assert(
  equal(flatten { arr1, arr3 }, { { 1, 2 }, 3, 1, { a = 1, 2 }, { b = 2 } })
)
assert(equal(flatten({ arr1, arr3 }, 'Deep'), { 1, 2, 3, 1, 2 }))
```

See also [extend](extend.md).
