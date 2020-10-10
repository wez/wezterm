# `wezterm.log_error(msg)`

This function logs the provided message string through wezterm's logging layer.
If you started wezterm from a terminal that text will print to the stdout of
that terminal.  If running as a daemon for the multiplexer server then it will
be logged to the daemon output path.

```lua
local wezterm = require 'wezterm';
wezterm.log_error("Hello!");
```


