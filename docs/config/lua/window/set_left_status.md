# `window:set_left_status(string)`

{{since('20220807-113146-c2fee766')}}

This method can be used to change the content that is displayed in the tab bar,
to the left of the tabs.  The content is displayed
left-aligned and will take as much space as needed to display the content
that you set; it will not be implicitly clipped.

The parameter is a string that can contain escape sequences that change
presentation.

It is recommended that you use [wezterm.format](../wezterm/format.md) to
compose the string.

See [window:set_right_status](set_right_status.md) for examples.

