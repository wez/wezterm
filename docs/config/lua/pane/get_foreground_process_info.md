# `pane:get_foreground_process_info()`

{{since('20220624-141144-bd1b7c5d')}}

Returns a [LocalProcessInfo](../LocalProcessInfo.md) object corresponding to the current foreground process that is running in the pane.

This method has some restrictions and caveats:

* This information is only available for local panes.  Multiplexer panes do not report this information.  Similarly, if you are using eg: `ssh` to connect to a remote host, you won't be able to access the name of the remote process that is running.
* On unix systems, the *process group leader* (the foreground process) will be queried, but that concept doesn't exist on Windows, so instead, the process tree of the originally spawned program is examined, and the most recently spawned descendant is assumed to be the foreground process
* On Linux, macOS and Windows, the process can be queried to determine this path. Other operating systems (notably, FreeBSD and other unix systems) are not currently supported
* Querying the path may fail for a variety of reasons outside of the control of WezTerm
* Querying process information has some runtime overhead, which may cause wezterm to slow down if over-used.

If the process cannot be determined then this method returns `nil`.

This example sets the right status to show the process id and executable path:

```lua
local wezterm = require 'wezterm'

-- Equivalent to POSIX basename(3)
-- Given "/foo/bar" returns "bar"
-- Given "c:\\foo\\bar" returns "bar"
function basename(s)
  return string.gsub(s, '(.*[/\\])(.*)', '%2')
end

wezterm.on('update-right-status', function(window, pane)
  local info = pane:get_foreground_process_info()
  if info then
    window:set_right_status(
      tostring(info.pid) .. ' ' .. basename(info.executable)
    )
  else
    window:set_right_status ''
  end
end)

return {}
```

