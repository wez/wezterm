# `wezterm.gui` module

{{since('20220807-113146-c2fee766')}}

The `wezterm.gui` module exposes functions that operate on the gui layer.

The multiplexer may not be connected to a GUI, so attempting to resolve
this module from the mux server will return `nil`.

You will typically use something like:

```lua
local wezterm = require 'wezterm'
local gui = wezterm.gui
if gui then
  -- do something that depends on the gui layer
end
```

## Available functions, constants


