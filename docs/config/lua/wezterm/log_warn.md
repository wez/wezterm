# `wezterm.log_warn(msg)`

*Since: nightly*

This function logs the provided message string through wezterm's logging layer
at 'WARN' level.  If you started wezterm from a terminal that text will print
to the stdout of that terminal.  If running as a daemon for the multiplexer
server then it will be logged to the daemon output path.

```lua
local wezterm = require 'wezterm';
wezterm.log_warn("Hello!");
```

See also [log_info](log_info.md) and [log_error](log_error.md).

