# `use_resize_increments = false`

{{since('20211204-082213-a66c61ee9')}}

When set to `true`, prefer to snap the window size to a multiple of the
terminal cell size. The default is `false`, which allows sizing the window to
an arbitrary size.

This option is only respected on X11, Wayland and macOS systems.

Note that if you have configured [window_padding](window_padding.md) then the
resize increments don't take the padding into account.

{{since('20240127-113634-bbcac864')}}

Window padding is now accounted for.
