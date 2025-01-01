# Command Line

This section documents the wezterm command line.

*Note that `wezterm --help` or `wezterm SUBCOMMAND --help` will show the precise
set of options that are applicable to your installed version of wezterm.*

wezterm is deployed with two major executables:

* `wezterm` (or `wezterm.exe` on Windows) - for interacting with wezterm from the terminal
* `wezterm-gui` (or `wezterm-gui.exe` on Windows) - for spawning wezterm from a desktop environment

You will typically use `wezterm` when scripting wezterm; it knows when to
delegate to `wezterm-gui` under the covers.

If you are setting up a launcher for wezterm to run in the Windows GUI
environment then you will want to explicitly target `wezterm-gui` so that
Windows itself doesn't pop up a console host for its logging output.

!!! note
    `wezterm-gui.exe --help` will not output anything to a console when
    run on Windows systems, because it runs in the Windows GUI subsystem and has no
    connection to the console.  You can use `wezterm.exe --help` to see information
    about the various commands; it will delegate to `wezterm-gui.exe` when
    appropriate.

## Synopsis

```console
{% include "../examples/cmd-synopsis-wezterm--help.txt" %}
```
