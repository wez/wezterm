# `window:set_position(x, y)`

*Since: nightly builds only*

Repositions the top-left corner of the window to the specified `x`, `y` coordinates.

Note that Wayland does not allow applications to directly control their window
placement, so this method has no effect on Wayland.
