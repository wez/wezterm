# `Pane` object

A Pane object cannot be created in lua code; it is typically passed to your
code via an event callback.  A Pane object is a handle to a live instance of a
Pane that is known to the wezterm process.  A Pane object tracks the psuedo
terminal (or real serial terminal) and associated process(es) and the parsed
screen and scrollback.

A Pane object can be used to send input to the associated processes and
introspect the state of the terminal emulation for that pane.

## Available methods


