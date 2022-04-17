# `window_decorations = "TITLE | RESIZE"`

*Since 20210314-114017-04b7cedd*

Configures whether the window has a title bar and/or resizable border.

The value is a set of of flags:

* `window_decorations = "NONE"` - disables titlebar and border (borderless mode)
* `window_decorations = "TITLE"` - disable the resizable border and enable only the title bar
* `window_decorations = "RESIZE"` - disable the title bar but enable the resizable border
* `window_decorations = "TITLE | RESIZE"` - Enable titlebar and border.  This is the default.

On X11 and Wayland, the windowing system may override the window decorations.

When the titlebar is disabled you can drag the window using the tab bar if it
is enabled, or by holding down `SUPER` and dragging the window (on Windows:
CTRL-SHIFT and drag the window).  You can map this dragging function for
yourself via the [StartWindowDrag](../keyassignment/StartWindowDrag.md) key
assignment.  Note that if the pane is running an application that has enabled
mouse reporting you will need to hold down the `SHIFT` modifier in order for
`StartWindowDrag` to be recognized.

When the resizable border is disabled you will need to use features of your
desktop environment to resize the window.  Windows users may wish to consider
[AltSnap](https://github.com/RamonUnch/AltSnap).

