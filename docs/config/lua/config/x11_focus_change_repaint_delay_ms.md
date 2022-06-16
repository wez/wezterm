## `x11_focus_change_repaint_delay_ms = 100`

*Since: nightly builds only*

When set to a non-zero value, WezTerm will wait the specified delay (in
milliseconds) after losing or gaining focus. It will then invalidate the
geometry and focus state and repaint the window.

The purpose of this option is to workaround a quirk when using proprietry
NVIDIA drivers together with EGL under X11: the quirk causes `CONFIGURE_NOTIFY`
to sometimes be suppressed, and `FOCUS_IN` and `FOCUS_OUT` events to sometimes
contain erroneous focus state.

