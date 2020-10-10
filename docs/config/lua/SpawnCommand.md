# SpawnCommand

The `SpawnCommand` struct specifies information about a new command
to be spawned.

It is a lua object with the following fields; all of the fields
have reasonable defaults and can be omitted.

```lua
{
  -- An optional label.
  -- The label is only used for SpawnCommands that are listed in
  -- the `launch_menu` configuration section.
  -- If the label is omitted, a default will be produced based
  -- on the `args` field.
  label = "List all the files!",

  -- The argument array specifying the command and its arguments.
  -- If omitted, the default program for the target domain will be
  -- spawned.
  args = {"ls", "-al"},

  -- The current working directory to set for the command.
  -- If omitted, wezterm will infer a value based on the active pane
  -- at the time this action is triggered.  If the active pane
  -- matches the domain specified in this `SpawnCommand` instance
  -- then the current working directory of the active pane will be
  -- used.
  -- If the current working directory cannot be inferred then it
  -- will typically fall back to using the home directory of
  -- the current user.
  cwd = "/some/path",

  -- Sets addditional environment variables in the environment for
  -- this command invocation.
  set_environment_variables = {
    SOMETHING = "a value"
  },

  -- Specifiy that the multiplexer domain of the currently active pane
  -- should be used to start this process.  This is usually what you
  -- want to happen and this is the default behavior if you omit
  -- the domain.
  domain = "CurrentPaneDomain",

  -- Specify that the default multiplexer domain be used for this
  -- command invocation.  The default domain is typically the "local"
  -- domain, which spawns processes locally.  However, if you started
  -- wezterm using `wezterm connect` or `wezterm serial` then the default
  -- domain will not be "local".
  domain = "DefaultDomain",

  -- Specify a named multiplexer domain that should be used to spawn
  -- this new command.
  -- This is useful if you want to assign a hotkey to always start
  -- a process in a remote multiplexer rather than based on the
  -- current pane.
  -- See the Multiplexing section of the docs for more on this topic.
  domain = {DomainName="my.server"},
}
```

