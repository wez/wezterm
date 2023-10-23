# `wezterm.table.to_string(table [, indent [, skip_outer_bracket]])`

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

*Note:* This function is not careful about checking for recursive tables, so it can't
be used to print e.g. `_G`. To print a recursive (or in general a very big) table,
it is recommended that you use [to_string_fallback](to_string_fallback.md).

By default this function constructs the string with 2 spaces for indentation.

The optional `indent` allows us to instead prefer other (non-negative) integer values
of spaces for the indentation.

```lua
local wezterm = require 'wezterm'
local tbl_to_string = wezterm.table.to_string

local tbl1 = {
  a = 1,
  {
    b = 2,
  },
}
local str1 = [[{
    a = 1,
    {
        b = 2,
    },
}]]

assert(str1 == tbl_to_string(tbl1, 4))
```

The optional `skip_outer_bracket` (which can only be used together with `indent`) is
a boolean, which defaults to `false`. If you set it to `true`, the outer brackets are
not included in the string (and thus everything is `indent` fewer spaces indented too).

```lua
local wezterm = require 'wezterm'
local tbl_to_string = wezterm.table.to_string

local tbl1 = {
  a = 1,
  {
    b = 2,
  },
}
local str1 = [[a = 1,
{
b = 2,
},]]

assert(str1 == tbl_to_string(tbl1, 0, true))
```

See also [to_string_fallback](to_string_fallback.md).
