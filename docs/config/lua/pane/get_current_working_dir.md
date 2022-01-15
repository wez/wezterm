# `pane:get_current_working_dir()`

*Since: 20201031-154415-9614e117*

Returns the current working directory of the pane, if known.
The current directory can be specified by an application sending
[OSC 7](../../../shell-integration.md).

If OSC 7 was never sent to a pane, and the pane represents a locally spawned process,
then wezterm will:

* On Unix systems, determie the *process group leader* attached to the PTY
* On Windows systems, use heuristics to infer an equivalent to the foreground process

With the process identified, wezterm will then try to determine the current
working directory using operating system dependent code:

|OS     |Supported?                            |
|-------|--------------------------------------|
|macOS  |Yes, *Since: 20201031-154415-9614e117*|
|Linux  |Yes, *Since: 20201031-154415-9614e117*|
|Windows|Yes, *Since: 20220101-133340-7edc5b5a*|

If the current working directory is not known then this method returns `nil`.
Otherwise, it returns the current working directory as a URI string.

