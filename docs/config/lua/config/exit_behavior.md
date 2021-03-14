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
