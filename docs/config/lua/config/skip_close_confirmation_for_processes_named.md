---
tags:
  - exit_behavior
---
# `skip_close_confirmation_for_processes_named`

{{since('20210404-112810-b63a949d')}}

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
config.skip_close_confirmation_for_processes_named = {
  'bash',
  'sh',
  'zsh',
  'fish',
  'tmux',
  'nu',
  'cmd.exe',
  'pwsh.exe',
  'powershell.exe',
}
```

More advanced control over this behavior can be achieved by defining a
[mux-is-process-stateful](../mux-events/mux-is-process-stateful.md) event handler.

