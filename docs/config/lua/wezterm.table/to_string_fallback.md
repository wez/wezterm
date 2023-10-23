# `wezterm.table.to_string_fallback(table)`

{{since('nightly')}}

This function takes a Lua table and returns a string with the data of
the table. E.g., passing in the table `{ a=1, b=2 }` the function
will return the string:
```
{
  ["a"] = 1,
  ["b"] = 2,
}
```

For nested tables, this function always prints a label (even for arrays).
This can make the string look different than you might expect.
```lua
local wezterm = require 'wezterm'
local tbl_to_string_fb = wezterm.table.to_string_fallback

local tbl1 = {
  a = 1,
  {
    b = 2,
  },
}
local str1 = [[{
  [1] = {
    ["b"] = 2,
  },
  ["a"] = 1,
}]]

assert(str1 == tbl_to_string_fb(tbl1))
```

See also [to_string](to_string.md).
