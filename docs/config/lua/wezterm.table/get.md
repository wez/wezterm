# `wezterm.table.get(table, key [, ...])`

{{since('nightly')}}

This function accepts a Lua table `table` and a key `key`.
It returns the value at the table entry `key` if `table` contains a key equal
to `key` (with non-nil value) and it returns `nil` otherwise.

The function accepts an optional arbitrary number of extra arguments, that will
all be intepreted as extra keys to check for recursively in the table. I.e., to
get the value of `table` at `table.a.b.c`, we can use
`wezterm.table.get(table, 'a', 'b', 'c')`.

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

assert(get(tbl1, 'a') == 1)
assert(get(tbl1, 'b') == tbl1.b) -- note: we get the table address of tbl1.b here
assert(get(tbl1, 'b', 'c', 'd') == 4)
assert(get(tbl1, 'c') == nil)

assert(get(arr1, 3) == 'c')
assert(get(arr1, 4) == nil)
assert(get(arr1, 1, 2) == nil)
```

See also [has_key](has_key.md) and [has_value](has_value.md).
