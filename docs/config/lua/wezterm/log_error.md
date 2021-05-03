# `wezterm.log_error(arg, ..)`

This function logs the provided message string through wezterm's logging layer
at 'ERROR' level.  If you started wezterm from a terminal that text will print
to the stdout of that terminal.  If running as a daemon for the multiplexer
server then it will be logged to the daemon output path.

```lua
local wezterm = require 'wezterm';
wezterm.log_error("Hello!");
```

*Since: nightly builds only*

Now accepts multiple arguments, and those arguments can be of any type.

See also [log_info](log_info.md) and [log_warn](log_warn.md).
