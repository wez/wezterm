---
title: wezterm.to_string
tags:
 - utility
---
# `wezterm.to_string(arg)`

{{since('nightly')}}

This function returns a string representation of any Lua value. In particular
this can be used to get a string representation of a table or userdata.

```lua
local wezterm = require 'wezterm'
assert(wezterm.to_string { 1, 2 } == [=[[
    1,
    2,
]]=])
assert(wezterm.to_string { a = 1, b = 2 } == [[{
    "a": 1,
    "b": 2,
}]])
```

