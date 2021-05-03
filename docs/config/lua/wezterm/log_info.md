# `wezterm.log_info(arg, ..)`

*Since: 20210314-114017-04b7cedd*

This function logs the provided message string through wezterm's logging layer
at 'INFO' level.  If you started wezterm from a terminal that text will print
to the stdout of that terminal.  If running as a daemon for the multiplexer
server then it will be logged to the daemon output path.

```lua
local wezterm = require 'wezterm';
wezterm.log_info("Hello!");
```

*Since: nightly builds only*

Now accepts multiple arguments, and those arguments can be of any type.


See also [log_error](log_error.md) and [log_warn](log_warn.md).

