*Since: 20220624-141144-bd1b7c5d*

The `wezterm.mux` module exposes functions that operate on the multiplexer layer.

The multiplexer manages the set of running programs into panes, tabs, windows
and workspaces.

The multiplexer may not be connected to a GUI so certain operations that require
a running Window management system are not present in the interface exposed
by this module.

You will typically use something like:

```lua
local wezterm = require 'wezterm'
local mux = wezterm.mux
```

at the top of your configuration file to access it.

## Important Note!

*You should **avoid using, at the file scope in your config**, mux functions that cause new splits, tabs or windows to be created. The configuration file can be evaluated multiple times in various contexts. If you want to spawn new programs when wezterm starts up, look at the [gui-startup](../gui-events/gui-startup.md) and [mux-startup](../mux-events/mux-startup.md) events.*

## Available functions, constants


