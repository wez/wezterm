# `wezterm cli list`

*Run `wezterm cli list --help` to see more help*

Lists the set of windows, tabs and panes that are being managed.

The default output is tabular:

```
$ wezterm cli list
WINID TABID PANEID WORKSPACE SIZE  TITLE                          CWD
    0     0      0 default   80x24 wezterm cli list  -- wez@foo:~ file://foo/home/wez/
```

Each row describes a pane.  The meaning of the fields are:

* `WINID` - the window id of the window that contains the pane
* `TABID` - the tab id of the tab that contains the pane
* `PANEID` - the pane id
* `WORKSPACE` - the workspace that the pane is associated with
* `SIZE` - the dimensions of the pane, measured in terminal cell columns x rows
* `TITLE` - the pane title
* `CWD` - the current working directory associated with the pane

{{since('20220624-141144-bd1b7c5d')}}

You may request JSON output:

```
$ wezterm cli list --format json
[
  {
    "window_id": 0,
    "tab_id": 0,
    "pane_id": 0,
    "workspace": "default",
    "size": {
      "rows": 24,
      "cols": 80
    },
    "title": "wezterm cli list --format json -- wez@foo:~",
    "cwd": "file://foo/home/wez/"
  }
]
```

## Synopsis

```console
{% include "../../examples/cmd-synopsis-wezterm-cli-list--help.txt" %}
```
