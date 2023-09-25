# `wezterm.table.clone(table)`

{{since('nightly')}}

This function clones the Lua table (or array) passed to it.

```lua
local wezterm = require 'wezterm'
local clone = wezterm.table.clone

local tbl1 = {
  a = 1,
  b = '2',
}

local tbl2 = clone(tbl1)

assert(tbl1 == tbl2)
```

