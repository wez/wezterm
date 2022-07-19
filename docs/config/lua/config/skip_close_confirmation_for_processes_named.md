# skip_close_confirmation_for_processes_named

*Since: 20210404-112810-b63a949d*

This configuration specifies a list of process names that are
considered to be "stateless" and that are safe to close without
prompting when closing windows, panes or tabs.

When closing a pane wezterm will try to determine the processes
that were spawned by the program that was started in the pane.
If all of those process names match one of the names in the
`skip_close_confirmation_for_processes_named` list then it will
not prompt for closing that particular pane.

The default value for this setting is shown below:

```lua
return {
  skip_close_confirmation_for_processes_named = {
    'bash',
    'sh',
    'zsh',
    'fish',
    'tmux',
  },
}
```

*Since: 20210814-124438-54e29167*:

This option now also works on Windows (prior versions only worked on Linux and
macOS), and the default value for this setting now includes some windows shell
processes:

```lua
return {
  skip_close_confirmation_for_processes_named = {
    'bash',
    'sh',
    'zsh',
    'fish',
    'tmux',
    'nu',
    'cmd.exe',
    'pwsh.exe',
    'powershell.exe',
  },
}
```

*Since: 20220101-133340-7edc5b5a*

More advanced control over this behavior can be achieved by defining a
[mux-is-process-stateful](../mux-events/mux-is-process-stateful.md) event handler.

