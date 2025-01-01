---
tags:
  - font
---

# `adjust_window_size_when_changing_font_size = true`

{{since('20210203-095643-70a364eb')}}

Control whether changing the font size adjusts the dimensions of the window
(true) or adjusts the number of terminal rows/columns (false). The default is
true.

If you use a tiling window manager then you may wish to set this to `false`.

See also [IncreaseFontSize](../keyassignment/IncreaseFontSize.md),
[DecreaseFontSize](../keyassignment/DecreaseFontSize.md).

{{since('20230712-072601-f4abf8fd')}}

The default value is now `nil` which causes wezterm to match the name of the
connected window environment (which you can see if you open the debug overlay)
against the list of known tiling environments configured by
[tiling_desktop_environments](tiling_desktop_environments.md).  If the
environment is known to be tiling then the effective value of
`adjust_window_size_when_changing_font_size` is `false`, and `true` otherwise.
