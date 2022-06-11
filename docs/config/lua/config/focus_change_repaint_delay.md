## `focus_change_repaint_delay = 100`

*Since: nightly builds only*

When not set to `0`, WezTerm will wait the specified delay (in milliseconds) before invalidating the geometry and repainting the window after losing or gaining focus.

Using the proprietry NVIDIA drivers and EGL rendering on X11, the `CONFIGURE_NOTIFY` event is sometimes missed. This workaround ensures that the window correctly resizes in these cases.

