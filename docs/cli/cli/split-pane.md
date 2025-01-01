# `wezterm cli split-pane`

*Run `wezterm cli split-pane --help` to see more help*

Split the current pane.
Outputs the pane-id for the newly created pane on success.

This command will create a split in the current pane and spawn a command into it.  This splits the pane and creates a new one at the bottom running the default command:

```
$ wezterm cli split-pane
2
```

You may spawn an alternative program by passing the argument list; it is
recommended that you use `--` to denote the end of the arguments being passed
to `wezterm cli split-pane` so that any parameters you may wish to pass to the
program are not confused with parameters to `wezterm cli split-pane`.  This example
launches bash as a login shell in a new pane at the bottom:

```
$ wezterm cli split-pane -- bash -l
3
```

This example creates a split to the left, occupying 30% of the available space:

```
$ wezterm cli split-pane --left --percent 30
4
```

The following options affect the behavior:

* `--cwd CWD` - Specify the current working directory for the initially spawned program.
* `--horizontal` - Equivalent to `--right`. If neither this nor any other direction is specified, the default is equivalent to `--bottom`.
* `--pane-id` - Specifies the pane that should be split. See also [Targeting Panes](index.md#targeting-panes).


{{since('20220624-141144-bd1b7c5d')}}

* `--bottom` - Split vertically, with the new pane on the bottom.
* `--cells CELLS` - The number of cells that the new split should have. If omitted, 50% of the available space is used.
* `--left` - Split horizontally, with the new pane on the left.
* `--move-pane-id MOVE_PANE_ID` - Instead of spawning a new command, move the specified pane into the newly created split.
* `--percent PERCENT` - Specify the number of cells that the new split should have, expressed as a percentage of the available space.
* `--right` - Split horizontally, with the new pane on the right.
* `--top` - Split vertically, with the new pane on the top.
* `--top-level` - Rather than splitting the active pane, split the entire window.

## Synopsis

```console
{% include "../../examples/cmd-synopsis-wezterm-cli-split-pane--help.txt" %}
```
