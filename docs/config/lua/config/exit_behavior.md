---
tags:
  - exit_behavior
---
## `exit_behavior = "CloseOnCleanExit"`

{{since('20210314-114017-04b7cedd')}}

Controls the behavior when the shell program spawned by the terminal exits.
There are three possible values:

* `"Close"` - close the corresponding pane as soon as the program exits.
* `"Hold"` - keep the pane open after the program exits. The pane must be manually closed via [CloseCurrentPane](../keyassignment/CloseCurrentPane.md), [CloseCurrentTab](../keyassignment/CloseCurrentTab.md) or closing the window.
* `"CloseOnCleanExit"` - if the shell program exited with a successful status, behave like `"Close"`, otherwise, behave like `"Hold"`.  This is the default setting.

```lua
console.exit_behavior = 'Hold'
```

Note that most unix shells will exit with the status of the last command that
it ran if you don't specify an exit status.

For example, if you interrupt a command and then use `exit` (with no arguments), or
CTRL-D to send EOF to the shell, the shell will return an unsuccessful exit
status.  The same thing holds if you were to run:

```console
$ false
$ exit
```

With the default `exit_behavior="CloseOnCleanExit"` setting, that will cause
the pane to remain open.

See also: [clean_exit_codes](clean_exit_codes.md) for fine tuning what is
considered to be a clean exit status.

{{since('20220624-141144-bd1b7c5d')}}

The default is now `"Close"`.

