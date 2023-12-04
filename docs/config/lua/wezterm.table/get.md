# `wezterm.table.get(table, key [, ...])`

{{since('nightly')}}

This function can be used to resolve the value for a key in a table. In its most basic form
it is equivalent to the built-in table indexing operator:
```lua
assert(wezterm.table.get(tbl, key) == tbl[key])
```
You may pass a sequence of keys that will be used to successively resolve
nested tables:
```
wezterm.table.get(tbl, 'a', 'b', 'c') == tbl['a']['b']['c']
```

*Note:* In the above `tbl['a']['b']['c']` might cause an error, since we might be indexing a nil value,
but `wezterm.table.get(tbl, 'a', 'b', 'c')` won't error in this case; instead it will return nil.


```lua
local wezterm = require 'wezterm'
local get = wezterm.table.get

local tbl1 = {
  a = 1,
  b = {
    c = {
      d = 4,
    },
  },
}

local arr1 = { 'a', 'b', 'c' }

assert(get(tbl1, 'a') == 1)
assert(get(tbl1, 'b') == tbl1.b) -- note: we get the table reference of tbl1.b here
assert(get(tbl1, 'b', 'c', 'd') == 4)
assert(get(tbl1, 'c') == nil)

assert(get(arr1, 3) == 'c')
assert(get(arr1, 4) == nil)
assert(get(arr1, 1, 2) == nil)
```

See also [has_key](has_key.md) and [has_value](has_value.md).
