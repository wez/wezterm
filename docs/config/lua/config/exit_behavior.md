## `exit_behavior = "CloseOnCleanExit"`

*Since: 20210314-114017-04b7cedd*

Controls the behavior when the shell program spawned by the terminal exits.
There are three possible values:

* `"Close"` - close the corresponding pane as soon as the program exits.
* `"Hold"` - keep the pane open after the program exits. The pane must be manually closed via [CloseCurrentPane](../keyassignment/CloseCurrentPane.md), [CloseCurrentTab](../keyassignment/CloseCurrentTab.md) or closing the window.
* `"CloseOnCleanExit"` - if the shell program exited with a successful status, behave like `"Close"`, otherwise, behave like `"Hold"`.  This is the default setting.

```lua
return {
  exit_behavior = "Hold",
}
```

Note that most unix shells will exit with the status of the last command that
it ran.  If you interrupt a command and then use CTRL-D to send EOF to the
shell, the shell will return an unsuccessful exit status.  With the default
`exit_behavior="CloseOnCleanExit"`, that will cause the pane to remain open.

