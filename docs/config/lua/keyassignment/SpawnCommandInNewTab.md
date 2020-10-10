## SpawnCommandInNewTab

Spawn a new tab into the current window.
The argument controls which command is run in the tab; it is a lua table
with the following fields:

* `args` - the argument array specifying the command and its arguments.
  If omitted, the default program will be run.
* `cwd` - the current working directory to set for the command.
* `set_environment_variables` - a table specifying key/value pairs to
  set in the environment
* `domain` - specifies the domain into which the tab will be spawned.
  See `SpawnTab` for examples.

```lua
local wezterm = require 'wezterm';

return {
  keys = {
    -- CMD-y starts `top` in a new window
    {key="y", mods="CMD", action=wezterm.action{SpawnCommandInNewWindow={
      args={"top"}
    }}},
  }
}
```


