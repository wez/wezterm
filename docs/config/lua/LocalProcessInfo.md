# `LocalProcessInfo`

{{since('20220101-133340-7edc5b5a')}}

`LocalProcessInfo` represents a process running on the local machine.

It has the following fields:

* `pid` - the process id
* `ppid` - the parent process id
* `name` - a short name for the process. Due to platform limitations, this may be inaccurate and truncated; you probably should prefer to look at the `executable` or `argv` fields instead of this one
* `status` - a string holding the status of the process; it can be `Idle`, `Run`, `Sleep`, `Stop`, `Zombie`, `Tracing`, `Dead`, `Wakekill`, `Waking`, `Parked`, `LockBlocked`, `Unknown`.
* `argv` - a table holding the argument array for the process
* `executable` - the full path to the executable image for the process (may be empty)
* `cwd` - the current working directory for the process (may be empty)
* `children` - a table keyed by child process id and whose values are themselves `LocalProcessInfo` objects that describe the child processes

See [mux-is-process-stateful](mux-events/mux-is-process-stateful.md) and [pane:get_foreground_process_info()](pane/get_foreground_process_info.md)
