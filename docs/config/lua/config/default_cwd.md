# `default_cwd`

*Since: nightly builds only*

Sets the default current working directory used by the initial window. If
`wezterm start --cwd /some/path` is used to specify the current working
directory, that will take precedence.

Commands launched using [`SpawnCommand`](../SpawnCommand.md) will use the
`cwd` specified in the `SpawnCommand`, if any.

Panes/Tabs/Windows created after the first will generally try to resolve the
current working directory of the current Pane, preferring
[a value set by OSC 7](../../../shell-integration.markdown) and falling back to
attempting to lookup the `cwd` of the current process group leader attached to a
local Pane. If no `cwd` can be resolved, then the `default_cwd` will be used.
If `default_cwd` is not specified, then the home directory of the user will be
used.

```text
                             Is initial window?
               ______________________|______________________
              |                                             |
             Yes                                            No
              |                                             |
       Opened with CLI                  New pane, tab, or window. Opened with a
      and `--cwd` flag?                   `SpawnCommand` that includes `cwd`?
    __________|__________                         __________|__________
   |                     |                       |                     |
  Yes                    No                      No                   Yes
   |                     |                       |                     |
  Use                    |                Is there a value    Use `cwd` specified
`--cwd`                  |                 set by OSC 7?       by `SpawnCommand`
               __________|             __________|__________
              |                       |                     |
              |                       No                   Yes
              |                       |                     |
              |            Can `cwd` be resolved via    Use OSC 7
              |            the process group leader?      value
              |             __________|__________
              |            |                     |
              |            No                   Yes
              |____________|                     |
                    |                       Use resolved
             Is `default_cwd`                  `cwd`
                 defined?
          __________|__________
         |                     |
        Yes                    No
         |                     |
        Use                 Use home
   `default_cwd`            directory
```

On macOS and Linux, `wezterm` can attempt to resolve the process group leader
and then attempt to resolve its current working directory. This is not
guaranteed to succeed, and there are a number of potential edge cases (which is
another reason for configuring your shell to use OSC 7 sequences).
