---
tags:
  - event
---
# `default_gui_startup_args = {"start"}`

{{since('20220101-133340-7edc5b5a')}}

When launching the GUI using either `wezterm` or `wezterm-gui` (with no
subcommand explicitly specified), wezterm will use the value of
`default_gui_startup_args` to pick a default mode for running the GUI.

The default for this config is `{"start"}` which makes `wezterm` with no
additional subcommand arguments equivalent to `wezterm start`.

If you know that you always want to use wezterm's ssh client to login to a
particular host, then you might consider using this configuration:

```lua
config.default_gui_startup_args = { 'ssh', 'some-host' }
```

which will cause `wezterm` with no additional subcommand arguments to be
equivalent to running `wezterm ssh some-host`.

Specifying subcommand arguments on the command line is NOT additive with
this config; the command line arguments always take precedence.

Depending on your desktop environment, you may find it simpler to use
your operating system shortcut or alias function to set up a shortcut
that runs the subcommand you desire.
