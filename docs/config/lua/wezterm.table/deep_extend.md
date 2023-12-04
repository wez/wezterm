# `wezterm.table.deep_extend(array_of_tables [, behavior])`

{{since('nightly')}}

This function merges a list of Lua object-style tables, producing a single object-style
table comprised of the keys of each of the tables in the input list, making a deep, recursive
copy of the corresponding value.  For a shallow copy, see [wezterm.table.extend](extend.md).

For each table in the `array_of_tables` parameter, the keys are iterated and set in
the return value.

The optional `behavior` parameter controls how repeated keys are handled; the
accepted values are:

* `"Force"` (this is the default) - always take the latest value for a key, even if
  the same key has already been populated into the return value, forcing the
  existing value to be updated with a later value.

* `"Keep"` - keep the first value of the key. Subsequent values for that same key
  are ignored.

* `"Error"` - when a key is seen more than once, raise an error.  This mode will
  only return if no keys are in conflict across the set of input tables.

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
  },
}

assert(
  equal(
    deep_extend { tbl1, tbl2 },
    { a = 2, b = { d = 4, e = 5 }, c = 3, d = 4 }
  )
)
assert(
  equal(
    deep_extend({ tbl1, tbl2 }, 'Keep'),
    { a = 1, b = { d = 4, e = 5 }, c = 3, d = 4 }
  )
)

local ok, msg = pcall(function() extend({tbl1, tbl2}, 'Error') end)
local msg_string = wezterm.to_string(msg)
wezterm.log_info(not ok and  msg_string:find "The key 'a' is in more than one of the tables." ~= nil)

assert(
  equal(
    deep_extend { tbl2, tbl3 },
    { a = 2, b = { a = 1, b = 2, e = 5 }, d = 4 }
  )
)
assert(
  equal(
    deep_extend({ tbl2, tbl3 }, 'Keep'),
    { a = 2, b = { a = 1, b = 2, e = 5 }, d = 4 }
  )
)
assert(
  equal(
    deep_extend({ tbl2, tbl3 }, 'Error'),
    { a = 2, b = { a = 1, b = 2, e = 5 }, d = 4 }
  )
)
```

See also [wezterm.table.flatten](flatten.md) and [wezterm.table.deep_extend](deep_extend.md).
