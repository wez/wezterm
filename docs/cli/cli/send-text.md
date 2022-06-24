# `wezterm cli send-text`

*Run `wezterm cli send-text --help` to see more help*

Send text to a pane as though it were pasted. If bracketed paste mode is
enabled in the pane, then the text will be sent as a bracketed paste.

For example:

```
$ wezterm cli send-text "hello there"
```

will cause `hello there` to be sent to the input in the current pane.

You can also pipe text in via stdin:

```
$ echo hello there | wezterm cli send-text
```

The following arguments modify the behavior:

* `--no-paste` - Send the text directly, rather than as a bracketed paste. (*Since: 20220624-141144-bd1b7c5d*)
* `--pane-id` - Specifies which pane to send the text to. See also [Targeting Panes](index.md#targeting-panes).
