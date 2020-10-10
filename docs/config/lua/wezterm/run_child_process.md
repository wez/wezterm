# `wezterm.run_child_process(args)`

*Since: 20200503-171512-b13ef15f*

This function accepts an argument list; it will attempt to spawn that command
and will return a tuple consisting of the boolean success of the invocation,
the stdout data and the stderr data.

```lua
local wezterm = require 'wezterm';

local success, stdout, stderr = wezterm.run_child_process({"ls", "-l"})
```


