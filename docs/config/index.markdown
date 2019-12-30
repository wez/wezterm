## Configuration Files

`wezterm` will look for a TOML configuration file in the following locations,
stopping at the first file that it finds:

* If the environment variable `$WEZTERM_CONFIG_FILE` is set, it will be treated as the
  path to a configuration file.
* On Windows, `wezterm.toml` from the directory that contains `wezterm.exe`.
  This is handy for users that want to carry their wezterm install around on a thumb drive.
* `$HOME/.config/wezterm/wezterm.toml`,
* `$HOME/.wezterm.toml`

`wezterm` will watch the config file that it loads;
if/when it changes, the configuration will be
automatically reloaded and the majority of options
will take effect immediately.  You may also use the
`CTRL+SHIFT+R` keyboard shortcut to force the configuration to be reloaded.

Configuration is currently very simple and the format is considered unstable and subject
to change.  The code for configuration can be found in [`src/config/mod.rs`](https://github.com/wez/wezterm/blob/master/src/config/mod.rs).

