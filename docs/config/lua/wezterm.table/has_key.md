# `wezterm.table.has_key(table, key [, ...])`

{{since('nightly')}}

This function accepts a Lua table `table` and a key `key`.
It returns `true` if `table` contains a key equal to `key` (with non-nil value)
and `false` otherwise.

The function accepts an optional arbitrary number of extra arguments, that will
all be intepreted as extra keys to check for recursively in the table. I.e., to
check whether `table` has any non-nil value at `table.a.b.c`, we can use
`wezterm.table.has_key(table, 'a', 'b', 'c')`.

```lua
local wezterm = require 'wezterm'
local has_key = wezterm.table.has_key

local tbl1 = {
  a = 1,
  b = {
    c = {
      d = 4
    }
  }
}

local arr1 = { 'a', 'b', 'c' }

assert(has_key(tbl1, 'a'))
assert(has_key(tbl1, 'b'))
assert(has_key(tbl1, 'b', 'c', 'd'))

assert(has_key(arr1, 3))
assert(not has_key(arr1, 4))
assert(not has_key(arr1, 1, 2))
```

See also [has_value](has_value.md) and [get](get.md).
