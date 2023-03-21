# `detect_password_input = true`

{{since('20220903-194523-3bb1ed61')}}

When set to `true`, on unix systems, for local panes, wezterm will query the
*termios* associated with the PTY to see whether local echo is disabled and
canonical input is enabled.

If those conditions are met, then the text cursor will be changed to a lock
to give a visual cue that what you type will not be echoed to the screen.

This technique only works for local processes on unix systems, and will not
work *through* other processes that themselves use PTYs. Most notably, this
will not work with tmux or remote processes spawned via ssh.

