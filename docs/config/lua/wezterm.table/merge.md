# `wezterm.table.merge(array_of_tables [, keep_first])`

{{since('nightly')}}

This function merges Lua tables passed to it in the form of an array.
I.e., to merge the Lua tables `tbl1` and `tbl2`, we can pass them to
the function as `{ tbl1, tbl2 }`. (See below.)

By default this function merges tables with identical keys by taking
the value from the last table in the array with each given key.

The optional `keep_first` allows us to instead prefer values from the
first table in the array where we see the key by passing `true` after the array.
The default behavior is identical to what we get by passing `false`.

```lua
local wezterm = require 'wezterm'
local merge = wezterm.table.merge

local tbl1 = {
  a = 1,
  b = '2',
}

local tbl2 = {
  a = '1',
  c = 3,
}

wezterm.log_error(merge { tbl1, tbl2 })
assert(merge { tbl1, tbl2 } == merge({ tbl1, tbl2 }, false))

wezterm.log_error(merge({ tbl1, tbl2 }, true))
```

See also [flatten](flatten.md).
