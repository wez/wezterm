# skip_close_confirmation_for_processes_named

*Since: 20210404-112810-b63a949d*

This only applies to linux and macOS systems.

This configuration specifies a list of process names that are
considered to be "stateless" and that are safe to close without
prompting when closing windows, panes or tabs.

When closing a pane wezterm will try to determine the processes
that were spawned by the program that was started in the pane.
If all of those process names matche one of the names in the
`skip_close_confirmation_for_processes_named` list then it will
not prompt for closing that particular pane.

The default value for this setting is shown below:

```
return {
  skip_close_confirmation_for_processes_named = {
    "bash", "sh", "zsh", "fish", "tmux"
  }
}
```

*Since: nightly builds only*:

The default value for this setting now includes some
windows shell processes:

```
return {
  skip_close_confirmation_for_processes_named = {
        "bash",
        "sh",
        "zsh",
        "fish",
        "tmux",
        "nu",
        "cmd.exe",
        "pwsh.exe",
        "powershell.exe"
  }
}
```
