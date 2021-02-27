## `exit_behavior = "Close"`

*Since: nightly builds only*

Controls the behavior when the shell program spawned by the terminal exits.
There are three possible values:

* `"Close"` - close the corresponding pane as soon as the program exits. This is the default setting.
* `"Hold"` - keep the pane open after the program exits. The pane must be manually closed via [CloseCurrentPane](../keyassignment/CloseCurrentPane.md), [CloseCurrentTab](../keyassignment/CloseCurrentTab.md) or closeing the window.
* `"CloseOnCleanExit"` - if the shell program exited with a successful status, behave like `"Close"`, otherwise, behave like `"Hold"`.
