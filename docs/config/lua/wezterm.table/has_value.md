# `wezterm.table.has_value(table, value [, behavior])`

{{since('nightly')}}

This function accepts a Lua table `table` and a value `value`.
It returns `true` if `table` contains an entry with value equal to `value`
and false otherwise. By default the function only searches for the value at
the top-level of the table.

The function accepts an optional string of the form `'Top'` or `'Deep'`
describing its behavior. Any other string passed to the function will result
in an error. The default behavior is equavalent to passing the string `'Top'`
as the behavior.

When `has_value` is run with the `'Top'` behavior, it will only go through the
top-level of the table to look for `value`. This is equavalent to going through
the table in Lua with `pairs` (not `ipairs`).

When `has_value` is run with the `'Deep'` behavior, it will recursively go through
all nested tables and look for `value` in each of them. It will return `true` if it
find `value` in any nested table, and otherwise it will return `false`.

```lua
local wezterm = require 'wezterm'
local has_value = wezterm.table.has_value

local tbl1 = {
  a = 1,
  b = {
    c = {
      d = 4
    }
  },
}
local arr1 = { 'a', 'b', 'c' }

assert(has_value(tbl1, 1))
assert(not has_value(tbl1, 4))
assert(has_value(tbl1, 4, 'Deep'))
assert(not has_value(tbl1, 'a', 'Deep'))

assert(has_value(arr1, 'a'))
assert(not has_value(arr1, '1'))
```

See also [has_key](has_key.md) and [get](get.md).
