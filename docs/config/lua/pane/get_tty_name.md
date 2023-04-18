# `pane:get_tty_name()`

{{since('20230408-112425-69ae8472')}}

Returns the tty device name, or `nil` if the name is unavailable.

* This information is only available for local panes.  Multiplexer panes do not report this information.  Similarly, if you are using eg: `ssh` to connect to a remote host, you won't be able to access the name of the remote process that is running.
* This information is only available on unix systems.  Windows systems do not have an equivalent concept.

This example sets the right status to show the tty name:

```lua
local wezterm = require 'wezterm'

wezterm.on('update-status', function(window, pane)
  local tty = pane:get_tty_name()
  if tty then
    window:set_right_status(tty)
  else
    window:set_right_status ''
  end
end)

return {}
```


