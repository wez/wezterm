## Configuration Files

`wezterm` will look for a [lua](https://www.lua.org/manual/5.3/manual.html)
configuration file in the following locations, stopping at the first file that
it finds:

* (since version 20210314-114017-04b7cedd) if the `--config-file` CLI argument was specified, then
  that path will be used.  If that path fails to load, then the defaults will be
  used instead.
* If the environment variable `$WEZTERM_CONFIG_FILE` is set, it will be treated as the
  path to a configuration file.  Since version 20210314-114017-04b7cedd: if that path fails to load
  then the defaults will be used instead.  In earlier releases, the following steps
  would be used as a fallback.
* On Windows, `wezterm.lua` from the directory that contains `wezterm.exe`.
  This is handy for users that want to carry their wezterm install around on a thumb drive.
* `$HOME/.config/wezterm/wezterm.lua`,
* `$HOME/.wezterm.lua`

`wezterm` will watch the config file that it loads; if/when it changes, the
configuration will be automatically reloaded and the majority of options will
take effect immediately.  You may also use the `CTRL+SHIFT+R` keyboard shortcut
to force the configuration to be reloaded.

**The configuration file may be evaluated multiple times for each wezterm
process** both at startup and in response to the configuration file being
reloaded.  You should avoid taking actions in the main flow of the config file
that have side effects; for example, unconditionally launching background
processes can result in many of them being spawned over time if you launch
many copies of wezterm, or are frequently reloading your config file.

### Configuration Overrides

*since: 20210314-114017-04b7cedd*

`wezterm` allows overriding configuration values via the command line; here are
a couple of examples:

```bash
$ wezterm --config enable_scroll_bar=true
$ wezterm --config 'exit_behavior="Hold"'
```

Configuration specified via the command line will always override the values
provided by the configuration file, even if the configuration file is reloaded.

Each window can have an additional set of window-specific overrides applied to
it by code in your configuration file.  That's useful for eg: setting
transparency or any other arbitrary option on a per-window basis.  Read the
[window:set_config_overrides](lua/window/set_config_overrides.md) documentation
for more information and examples of how to use that functionality.

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



