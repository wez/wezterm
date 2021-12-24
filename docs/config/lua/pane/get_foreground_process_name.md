# `pane:get_foreground_process_name()`

*Since: nightly builds only*

Returns the path to the executable image for the pane.

This method has some restrictions and caveats:

* This information is only available for local panes.  Multiplexer panes do not report this information.  Similarly, if you are using eg: `ssh` to connect to a remote host, you won't be able to access the name of the remote process that is running.
* On unix systems, the *process group leader* (the foreground process) will be queried, but that concept doesn't exist on Windows, so instead, the program that was used to launch the pane will be used
* On Linux, macOS and Windows, the process can be queried to determine this path. Other operating systems (notably, FreeBSD and other unix systems) are not currently supported
* Querying the path may fail for a variety of reasons outside of the control of WezTerm
* Querying process information has some runtime overhead, which may cause wezterm to slow down if over-used.

If the path is not known then this method returns `nil`.

This example sets the right status are to the executable path:

```lua
local wezterm = require 'wezterm'

wezterm.on("update-right-status", function(window, pane)
  window:set_right_status(pane:get_foreground_process_name())
end)

return {
}
```
