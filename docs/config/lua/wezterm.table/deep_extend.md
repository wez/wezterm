# `wezterm.table.deep_extend(array_of_tables [, behavior])`

{{since('nightly')}}

This function merges a list of Lua object-style tables based on their keys in
each nested table.

The tables are passed to it in the form of an array.
I.e., to merge the Lua tables `tbl1` and `tbl2`, we can pass them to
the function as `{ tbl1, tbl2 }`. (See below.)

By default this function merges tables with identical keys for non-table values
by taking the value from the last table in the array with each given key.

The function accepts an optional string of the form `'Keep'`, `'Force'` or
`'Error` describing its behavior. Any other string passed to the function will
result in an error. The default behavior is equavalent to passing the string
`'Force'`as the behavior.

When `deep_extend` is run with the `'Keep'` behavior, it will prefer values from the
first table in the array where we see the key. (In contrast to `'Force'` that
prefers later values.)

When `extend` is run with the `'Error'` behavior, it will return an error if
any of the tables passed to it contain the same key for a non-table value, and it
will not try to merge the tables in this case. Otherwise, it will cleanly merge the
tables with no ambiguity, since there are no duplicate keys with non-table values.

```lua
local wezterm = require 'wezterm'
local deep_extend = wezterm.table.deep_extend
local equal = wezterm.table.equal

local tbl1 = {
  a = 1,
  b = {
    d = 4,
  },
  c = 3,
}

local tbl2 = {
  a = 2,
  b = {
    e = 5,
  },
  d = 4,
}

local tbl3 = {
  b = {
    a = 1,
    b = 2,
  }
}

assert(equal(deep_extend({tbl1, tbl2}), { a = 2, b = { d = 4, e = 5 }, c = 3, d = 4 }))
assert(equal(deep_extend({tbl1, tbl2}, 'Keep'), { a = 1, b = { d = 4, e = 5 }, c = 3, d = 4 }))
-- This will return an error: deep_extend({tbl1, tbl2}, 'Error')

assert(equal(deep_extend({tbl2, tbl3}), { a = 2, b = { a = 1, b = 2, e = 5 }, d = 4 }))
assert(equal(deep_extend({tbl2, tbl3}, 'Keep'), { a = 2, b = { a = 1, b = 2, e = 5 }, d = 4 }))
assert(equal(deep_extend({tbl2, tbl3}, 'Error'), { a = 2, b = { a = 1, b = 2, e = 5 }, d = 4 }))
```

See also [flatten](flatten.md) and [deep_extend](deep_extend.md).
