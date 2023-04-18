# `wezterm cli get-pane-direction DIRECTION`

{{since('20230408-112425-69ae8472')}}

*Run `wezterm cli get-pane-direction --help` to see more help*

Prints the pane id of the pane in the specified direction, relative to
the current pane.

Possible values for `DIRECTION` are shown below; the direction is matched
ignoring case so you can use `left` rather than `Left`:

* `Left`, `Right`, `Up`, `Down` based on direction
* `Next`, `Prev` based on the ordinal position in the pane tree.

## Synopsis

```console
{% include "../../examples/cmd-synopsis-wezterm-cli-get-pane-direction--help.txt" %}
```

