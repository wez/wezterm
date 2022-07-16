# `wezterm.procinfo.executable_path_for_pid(pid)`

*Since: nightly builds only*

Returns the path to the executable image for the specified process id.

This function may return `nil` if it was unable to return the info.

```lua
> wezterm.procinfo.executable_path_for_pid(wezterm.procinfo.pid())
"/home/wez/wez-personal/wezterm/target/debug/wezterm-gui"
```

