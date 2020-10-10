## Lua Reference

This section documents the various lua functions that are provided to
the configuration file.  These are provided by the `wezterm` module that
must be imported into your configuration file:

```lua
local wezterm = require 'wezterm';
return {
  font = wezterm.font("JetBrains Mono"),
}
```

### Making your own Lua Modules

If you'd like to break apart your configuration into multiple files, you'll
be interested in this information.

The `package.path` is configured with the following paths in this order:

* On Windows: a `wezterm_modules` dir in the same directory as `wezterm.exe`
* `~/.config/wezterm`
* `~/.wezterm`
* A system specific set of paths which may (or may not!) find locally installed lua modules

That means that if you wanted to break your config up into a `helpers.lua` file
you would place it in `~/.config/wezterm/helpers.lua` with contents like this:

```lua
-- I am helpers.lua and I should live in ~/.config/wezterm/helpers.lua

local wezterm = require 'wezterm';

-- This is the module table that we will export
local module = {}

-- This function is private to this module and is not visible
-- outside.
local function private_helper()
  wezterm.log_error("hello!")
end

-- define a function in the module table.
-- Only functions defined in `module` will be exported to
-- code that imports this module
function module.my_function()
  private_helper()
end

-- return our module table
return module
```

and then in your `wezterm.lua`
you would use it like this:

```lua
local helpers = require 'helpers';
helpers.my_function()
```
