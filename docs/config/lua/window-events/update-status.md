# `update-status`

{{since('20220903-194523-3bb1ed61')}}

The `update-status` event is emitted periodically (based on the
interval specified by the [status_update_interval](../config/status_update_interval.md)
configuration value).

There is no defined return value for the event, but its purpose is to allow
you the chance to carry out some activity and then ultimately call
[window:set_right_status](../window/set_right_status.md) or [window:set_left_status](../window/set_left_status.md).

The first event parameter is a [`window` object](../window/index.md) that
represents the gui window.

The second event parameter is a [`pane` object](../pane/index.md) that
represents the active pane in that window.

`wezterm` will ensure that only a single instance of this event is outstanding;
if the hook takes longer than the
[status_update_interval](../config/status_update_interval.md) to complete,
`wezterm` won't schedule another call until `status_update_interval`
milliseconds have elapsed since the last call completed.


