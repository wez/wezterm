# `wezterm cli spawn`

*Run `wezterm cli spawn --help` to see more help*

Spawn a command into a new tab or window.  Outputs the pane-id for the newly
created pane on success.

When run with no arguments, it will spawn a new tab running the default
program; this example spawns a new pane with id 1 running that default program
(most likely: your shell):


```
$ wezterm cli spawn
1
```

You may spawn an alternative program by passing the argument list; it is
recommended that you use `--` to denote the end of the arguments being passed
to `wezterm cli spawn` so that any parameters you may wish to pass to the
program are not confused with parameters to `wezterm cli spawn`.  This example
launches `top` in a new tab:

```
$ wezterm cli spawn -- top
2
```

This example explicitly runs bash as a login shell:

```
$ wezterm cli spawn -- bash -l
3
```

The following options affect the behavior:

* `--cwd CWD` - Specifies the current working directory that should be set for the spawned program
* `--domain-name DOMAIN_NAME` - Spawn into the named multiplexer domain. The default is to spawn into the domain of the current pane.
* `--new-window` - Spawns the tab into a window of its own.
* `--workspace WORKSPACE` - when using `--new-window`, set the workspace name rather than using the default name of `"default"`.
* `--window-id WINDOW_ID` - Spawn the tab into the specified window, rather than using the current window


## Synopsis

```console
{% include "../../examples/cmd-synopsis-wezterm-cli-spawn--help.txt" %}
```
