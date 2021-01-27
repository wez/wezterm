# `default_clipboard_paste_source`

*Since: nightly builds only*

Specifies which clipboard buffer should be read by the
[Paste](../keyassignment/Paste.md) action.

Possible values are:

* `Clipboard` - paste from the system clipboard
* `PrimarySelection` - paste from the primary selection buffer (applicable to X11 systems only)

The default value is `Clipboard`.

Prior to the introduction of this option, the behavior was not configurable,
but had the same behavior as the default.
