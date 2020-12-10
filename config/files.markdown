## Configuration Files

`wezterm` will look for a [lua](https://www.lua.org/manual/5.3/manual.html)
configuration file in the following locations, stopping at the first file that
it finds:

* If the environment variable `$WEZTERM_CONFIG_FILE` is set, it will be treated as the
  path to a configuration file.
* On Windows, `wezterm.lua` from the directory that contains `wezterm.exe`.
  This is handy for users that want to carry their wezterm install around on a thumb drive.
* `$HOME/.config/wezterm/wezterm.lua`,
* `$HOME/.wezterm.lua`

`wezterm` will watch the config file that it loads; if/when it changes, the
configuration will be automatically reloaded and the majority of options will
take effect immediately.  You may also use the `CTRL+SHIFT+R` keyboard shortcut
to force the configuration to be reloaded.

## Configuration File Structure

The `wezterm.lua` configuration file is a lua script which allows for a high
degree of flexibility.   The script is expected to return a configuration
table, so a basic empty configuration file will look like this:

```lua
return {
}
```

Throughout these docs you'll find configuration fragments that demonstrate
configuration and that look something like this:

```lua
return {
  color_scheme = "Batman",
}
```

and perhaps another one like this:

```lua
local wezterm = require 'wezterm';
return {
  font = wezterm.font("JetBrains Mono"),
}
```

If you wanted to use both of these in the same file, you would merge them together
like this:

```lua
local wezterm = require 'wezterm';
return {
  font = wezterm.font("JetBrains Mono"),
  color_scheme = "Batman",
}
```



