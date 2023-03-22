# Lua Reference

This section documents the various lua functions and types that are provided to
the configuration file.  These are provided by the `wezterm` module that must
be imported into your configuration file:

```lua
local wezterm = require 'wezterm'
local config = {}
config.font = wezterm.font 'JetBrains Mono'
return config
```

