# `wezterm.procinfo.current_working_dir_for_pid(pid)`

*Since: nightly builds only*

Returns the current working directory for the specified process id.

This function may return `nil` if it was unable to return the info.

```lua
> wezterm.procinfo.current_working_dir_for_pid(wezterm.procinfo.pid())
"/home/wez/wez-personal/wezterm"
```

