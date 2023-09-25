# `wezterm.table.to_string(table [, indent])`

{{since('nightly')}}

This function takes a Lua table and returns a string with the data of
the table. E.g., passing in the table `{ a=1, b=2 }` the function
will return the string:
```
{
    a = 1,
    b = 2,
}
```

By default this function constructs the string with 4 spaces for indentation.

The optional `indent` allows us to instead prefer other (positive) integer values
of spaces for the indentation.

```lua
local wezterm = require 'wezterm'
local to_string = wezterm.table.to_string

local tbl1 = {
  a = 1,
  b = 2,
}
local str1 = [[{
    a = 1,
    b = 2,
}]]

assert(str1 == to_string(tbl1))
```

