# `wezterm.action`

Helper for defining key assignment actions in your configuration file.
This is really just sugar for the underlying Lua -> Rust deserialation
mapping that makes it a bit easier to identify where syntax errors may
exist in your configuration file.

Usage looks like this:

```lua
local wezterm = require 'wezterm';
return {
   keys = {
     {key="{", mods="CTRL", action=wezterm.action{ActivateTabRelative=-1}},
     {key="}", mods="CTRL", action=wezterm.action{ActivateTabRelative=1}},
   }
}
```

The parameter is a lua representation of the underlying
[KeyAssignment](https://github.com/wez/wezterm/blob/master/config/src/keyassignment.rs#L114)
enum from the configuration code.  These docs aim to spell out sufficient
examples that you shouldn't need to learn to read Rust code, but there
are occasions where newly developed features are not yet documented and
an enterprising user may wish to go spelunking to figure them out!

