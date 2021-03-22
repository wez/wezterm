# skip_close_confirmation_for_processes_named

*Since: nightly*

This currently only applies to linux systems.

This configuration specifies a list of process names that are
considered to be "stateless" and that are safe to close without
prompting when closing windows, panes or tabs.

When closing a pane wezterm will try to determine the foreground
process in that pane.  If it matches one of the names in the
`skip_close_confirmation_for_processes_named` list then it will
not prompt for closing that particular pane.

The mechanism used for this can only inspect the foreground
process, so if you have a backgrounded editor then wezterm
will only see the foreground shell and decide that it is
ok to skip prompting.

The default value for this setting is shown below:

```
return {
  skip_close_confirmation_for_processes_named = {
    "bash", "sh", "zsh", "fish", "tmux"
  }
}
```
