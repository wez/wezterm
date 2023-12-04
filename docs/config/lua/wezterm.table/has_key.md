# `wezterm.table.has_key(table, keys..)`

{{since('nightly')}}

This function can be used to check if a key in a table has a non-nil value. In its most
basic form it is equivalent to:
```lua
assert(wezterm.table.has_key(tbl, key) == (tbl[key] ~= nil))
```
You may pass a sequence of keys that will be used to successively check nested tables:
```lua
local tbl = { a = { b = { c = true } } }
assert(wezterm.table.has_key(tbl, 'a', 'b', 'c') == (tbl['a']['b']['c'] ~= nil))
```

*Note:*

* In the above `tbl['a']['a']['a']` would cause an error, since we are indexing a nil value,
  but `wezterm.table.has_key(tbl, 'a', 'a', 'a')` won't error in this case; instead it will return
  `false`.
* This function can also be used on `Userdata` objects that implement an `__index` metamethod.

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
