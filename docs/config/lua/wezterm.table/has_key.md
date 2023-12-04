# `wezterm.table.has_key(table, keys..)`

{{since('nightly')}}

This function accepts a Lua table `table` and an arbitrary number of keys `keys..`.
It returns `true` if `table` contains a non-nil value at the position specified by
`keys..` and `false` otherwise.

The arbitrary number of extra arguments will all be intepreted as extra keys to check
for recursively in the table. I.e., to check whether `table` has any non-nil value at
`table['a']['b']['c']`, we can use `wezterm.table.has_key(table, 'a', 'b', 'c')`.

*Note:*

* In the above `table['a']['b']['c']` might cause an error, since we might be indexing a nil value,
  but `wezterm.table.has_key(table, 'a', 'b', 'c')` won't error in this case; instead it will return `false`.
* This function can also be used on `Userdata` objects that implement an `__index`
  metamethod.

```lua
local wezterm = require 'wezterm'
local has_key = wezterm.table.has_key

local tbl1 = {
  a = 1,
  b = {
    c = {
      d = 4,
    },
  },
}

local arr1 = { 'a', 'b', 'c' }

assert(has_key(tbl1, 'a'))
assert(has_key(tbl1, 'b'))
assert(has_key(tbl1, 'b', 'c', 'd'))

assert(has_key(arr1, 3))
assert(not has_key(arr1, 4))
assert(not has_key(arr1, 1, 2))
```

See also [wezterm.table.has_value](has_value.md) and [wezterm.table.get](get.md).
