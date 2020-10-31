# `pane:get_current_working_dir()`

*Since: 20201031-154415-9614e117*

Returns the current working directory of the pane, if known.
The current directory can be specified by an application sending
[OSC 7](../../../shell-integration.md).

On Linux and macOS, if OSC 7 was never sent to the pane, wezterm will attempt
to inspect the cwd of the process group leader attached to the pty and use
that.

If the current working directory is not known then this method returns `nil`.
Otherwise, it returns the current working directory as a URI string.
