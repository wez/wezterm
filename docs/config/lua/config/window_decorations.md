---
tags:
  - appearance
---
# `window_decorations = "TITLE | RESIZE"`

{{since('20210314-114017-04b7cedd')}}

Configures whether the window has a title bar and/or resizable border.

The value is a set of flags:

* `window_decorations = "NONE"` - disables titlebar and border (borderless
  mode), but causes problems with resizing and minimizing the window, so you
  probably want to use `RESIZE` instead of `NONE` if you just want to remove
  the title bar.
* `window_decorations = "TITLE"` - disable the resizable border and enable only the title bar
* `window_decorations = "RESIZE"` - disable the title bar but enable the resizable border
* `window_decorations = "TITLE | RESIZE"` - Enable titlebar and border.  This is the default.

{{since('20230320-124340-559cb7b0', outline=true)}}
    The following flags are also supported on macOS:

    * `MACOS_FORCE_DISABLE_SHADOW` - disable the window shadow effect
    * `MACOS_FORCE_ENABLE_SHADOW` - enable the window shadow effect.

    The window shadow effect is normally disabled by wezterm when the
    [window_background_opacity](../../appearance.md#window-background-opacity) is set
    to less than `1.0`.

{{since('20230408-112425-69ae8472', outline=true)}}
    * `window_decorations = "INTEGRATED_BUTTONS|RESIZE"` - place window
      management buttons (minimize, maximize, close) into the tab bar
      instead of showing a title bar.

      See also [integrated_title_button_style](integrated_title_button_style.md),
      [integrated_title_buttons](integrated_title_buttons.md),
      [integrated_title_button_alignment](integrated_title_button_alignment.md)
      [integrated_title_button_color](integrated_title_button_color.md) and,
      if you are using the retro tab bar, [tab_bar_style](tab_bar_style.md).

{{since('nightly', outline=true)}}
    The following flag is also supported:

    * `MACOS_FORCE_SQUARE_CORNERS` - on macOS, force the window to have square
      rather than rounded corners. It is not compatible with `TITLE` or
      `INTEGRATED_BUTTONS`

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

!!! warning
    Think twice before removing `RESIZE` from the set of decorations as it causes
    problems with resizing and minimizing the window. You usually want to keep
    `RESIZE` enabled.

!!! danger
    If you just want to remove the title bar, set `window_decorations = "RESIZE"`
    as you will run into problems if you remove `RESIZE` from the set of
    decorations.

!!! tip
    You probably always want `RESIZE` to be listed in your `window_decorations`.

