# wezterm.add_to_config_reload_watch_list(path)

*Since: nightly builds only*

Adds `path` to the list of files that are watched for config changes.
If [automatically_reload_config](../config/automatically_reload_config.md)
is enabled, then the config will be reloaded when any of the files
that have been added to the watch list have changed.

The intent of for this to be used together with a custom lua loader
in a future iteration of wezterm.
