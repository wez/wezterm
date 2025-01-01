---
tags:
  - spawn
  - launch_menu
---
# `launch_menu`

{{since('20200503-171512-b13ef15f')}}

You can define your own entries for the [Launcher Menu](../../launch.md#the-launcher-menu)
using this configuration setting.  The snippet below adds two new entries to
the menu; one that runs the `top` program to monitor process activity and a
second one that explicitly launches the `bash` shell.

Each entry in `launch_menu` is an instance of a
[SpawnCommand](../SpawnCommand.md) object.

```lua
config.launch_menu = {
  {
    args = { 'top' },
  },
  {
    -- Optional label to show in the launcher. If omitted, a label
    -- is derived from the `args`
    label = 'Bash',
    -- The argument array to spawn.  If omitted the default program
    -- will be used as described in the documentation above
    args = { 'bash', '-l' },

    -- You can specify an alternative current working directory;
    -- if you don't specify one then a default based on the OSC 7
    -- escape sequence will be used (see the Shell Integration
    -- docs), falling back to the home directory.
    -- cwd = "/some/path"

    -- You can override environment variables just for this command
    -- by setting this here.  It has the same semantics as the main
    -- set_environment_variables configuration option described above
    -- set_environment_variables = { FOO = "bar" },
  },
}
```

