# `default_clipboard_copy_destination`

*Since: nightly builds only*

Specifies which clipboard buffer should be populated by the [Copy](../keyassignment/Copy.md), [CompleteSelection](../keyassignment/CompleteSelection.md) and [CompleteSelectionOrOpenLinkAtMouseCursor](../keyassignment/CompleteSelectionOrOpenLinkAtMouseCursor.md) actions.

Possible values are:

* `Clipboard` - copy the text to the system clipboard.
* `PrimarySelection` - Copy the test to the primary selection buffer (applicable to X11 systems only)
* `ClipboardAndPrimarySelection` - Copy to both the clipboard and the primary selection.

The default value is `ClipboardAndPrimarySelection`.

Prior to the introduction of this option, the behavior was not configurable,
but had the same behavior as the default.
