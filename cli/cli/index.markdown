# `wezterm cli`

The *cli* subcommand interacts with a running wezterm GUI or multiplexer
instance, and can be used to spawn programs and manipulate tabs and panes.

# Targeting the correct instance

There may be multiple GUI processes running in addition to a multiplexer server.
wezterm uses the following logic to decide which one it should connect to.

* If the `--prefer-mux` flag is passed, then the `wezterm.lua` config file is
  consulted to determine the first *unix domain* defined by the config.
* If the `$WEZTERM_UNIX_SOCKET` environment variable is set, use that location
  to identify the running instance
* Try to locate a running GUI instance. The `--class` argument specifies an
  optional window class that can be used to select the appropriate GUI window
  if that GUI window was also spawned using `--class` to override the default.

# Targeting Panes

Various subcommands target panes via a (typically optional) `--pane-id` argument.

The following rules are used to determine a pane if `--pane-id` is not specified:

* If the `$WEZTERM_PANE` environment variable is set, it will be used
* The list of clients is retrieved and sorted by the most recently interacted
  session. The focused pane id from that session is used

See also: [wezterm cli list](list.md)

# Available Subcommands

