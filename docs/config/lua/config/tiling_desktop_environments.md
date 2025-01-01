# `tiling_desktop_environments = {}`

{{since('20230712-072601-f4abf8fd')}}

Contains a list of Window Environments that are known to be tiling
window managers. A tiling window manager is one that automatically
resizes windows according to some layout policy, rather than respecting
the window size set by an application.

The default value for this option is:

```lua
config.tiling_desktop_environments = {
  'X11 LG3D',
  'X11 bspwm',
  'X11 i3',
  'X11 dwm',
}
```

{{since('dev')}}

The following additional entries are now part of the default value of
`tiling_desktop_environments`:

 *  '"X11 awesome"'

The environment name can be found in the debug overlay which you can show via
the [ShowDebugOverlay](../keyassignment/ShowDebugOverlay.md) key assignment.
The default key binding for it is <kbd>Ctrl</kbd> + <kbd>Shift</kbd> +
<kbd>L</kbd>.

Look for the line beginning with `Window Environment:`. The text after the
colon is the name to add to `tiling_desktop_environments`.

If your window environment is a tiling environment and is not listed
here, please file an issue (or even a PR!) to add it to the default
list.

This contents of this list are used to determine a reasonable default for
[adjust_window_size_when_changing_font_size](adjust_window_size_when_changing_font_size.md).

