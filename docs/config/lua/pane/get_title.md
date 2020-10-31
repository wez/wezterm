# `pane:get_title()`

*Since: 20201031-154415-9614e117*

Returns the title of the pane.  This will typically be `wezterm` by default but
can be modified by applications that send `OSC 1` (Icon/Tab title changing)
and/or `OSC 2` (Window title changing) escape sequences.

The value returned by this method is the same as that used to display the
tab title if this pane were the only pane in the tab; if `OSC 1` was used
to set a non-empty string then that string will be returned.  Otherwise the
value for `OSC 2` will be returned.

Note that on Microsoft Windows the default behavior of the OS level PTY is to
implicitly send `OSC 2` sequences to the terminal as new programs attach to the
console.
