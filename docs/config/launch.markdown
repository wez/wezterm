## Launching Programs

By default, when opening new tabs or windows, your shell will be spawned.

Your shell is determined by the following rules:

### On Posix Systems

1. The value of the `$SHELL` environment variable is used if it is set
2. Otherwise, it will resolve your current uid and try to look up your
   home directory from the password database.

`wezterm` will spawn the shell and pass `-l` as an argument to request
a login shell.  A login shell generally loads additional startup files
and sets up more environment than a non-login shell.

Note: if you have recently changed your shell using `chsh` and you
have `$SHELL` set in the environment, you will need to sign out and
sign back in again for the environment to pick up your new `$SHELL`
value.

### On Windows Systems

1. The value of the `%COMSPEC%` environment variable is used if it is set.
2. Otherwise, `cmd.exe`

## Changing the default program

If you'd like `wezterm` to run a different program than the shell as
described above, you can use the `default_prog` config setting to specify
the argument array; the array allows specifying the program and arguments
portably:

```lua
return {
  -- Spawn a fish shell in login mode
  default_prog = {"/usr/local/bin/fish", "-l"},
}
```

## Passing Environment variables to the spawned program

The `set_environment_variables` configuration setting can be used to
add environment variables to the environment of the spawned program.

The behavior is to take the environment of the `wezterm` process
and then set the specified variables for the spawned process.

```lua
return {
  set_environment_variables = {
    -- This changes the default prompt for cmd.exe to report the
    -- current directory using OSC 7, show the current time and
    -- the current directory colored in the prompt.
    prompt = "$E]7;file://localhost/$P$E\\$E[32m$T$E[0m $E[35m$P$E[36m$_$G$E[0m "
  },
}
```

# The Launcher Menu

The launcher menu is accessed from the new tab button in the tab bar UI; the
`+` button to the right of the tabs.  Left clicking on the button will spawn
a new tab, but right clicking on it will open the launcher menu.  You may also
bind a key to the `ShowLauncher` action to trigger the menu.

The launcher menu by default lists the various multiplexer domains and offers
the option of connecting and spawning tabs/windows in those domains.

*(New in the most recent nightly!)* You can define you own entries using the
`launch_menu` configuration setting.  The snippet below adds two new entries to
the menu; one that runs the `top` program to monitor process activity and a
second one that explicitly launches the `bash` shell.

```lua
return {
  launch_menu = {
    {
      args = {"top"},
    },
    {
      -- Optional label to show in the launcher. If omitted, a label
      -- is derived from the `args`
      label = "Bash",
      -- The argument array to spawn.  If omitted the default program
      -- will be used as described in the documentation above
      args = {"bash", "-l"},

      -- You can specify an alternative current workding directory;
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
}
```

<img src="../screenshots/launch-menu.png" alt="Screenshot">

