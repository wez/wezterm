# `wezterm cli get-text`

{{since('20230320-124340-559cb7b0')}}

*Run `wezterm cli get-text --help` to see more help*

Retrieves the textual content of a pane and output it to stdout.

For example:

```
$ wezterm cli get-text > /tmp/myscreen.txt
```

will capture the main (non-scrollback) portion of the current pane to `/tmp/myscreen.txt`.

By default, just the raw text is captured without any color or styling escape sequences.
You may pass `--escapes` to include those:

```
$ wezterm cli get-text --escapes > /tmp/myscreen-with-colors.txt
```

The default capture region is the main terminal screen, not including the scrollback.
You may use the `--start-line` and `--end-line` parameters to set the range.
Both of these accept integer values, where `0` refers to the top of the non-scrollback
screen area, and negative numbers index backwards into the scrollback.

## Synopsis

```console
{% include "../../examples/cmd-synopsis-wezterm-cli-get-text--help.txt" %}
```
