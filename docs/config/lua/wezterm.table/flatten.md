# `wezterm.table.flatten(array_of_arrays)`

{{since('nightly')}}

This function flattens Lua arrays passed to it in the form of an array.
I.e., to flatten the Lua arrays `arr1` and `arr2` into one array,
we can pass them to the function as `{ arr1, arr2 }`. (See below.)

```lua
local wezterm = require 'wezterm'
local flatten = wezterm.table.flatten

local arr1 = { 1, 2, 3 }

local arr2 = { 'a', 'b', 'c' }

wezterm.log_error(flatten { arr1, arr2 })
```

See also [merge](merge.md).
