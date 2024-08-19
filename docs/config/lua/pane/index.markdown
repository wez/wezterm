# `Pane` object

---
{{since('20221119-145034-49b9839f')}}

In previous releases there were separate `MuxPane` and `Pane` objects created
by the mux and gui layers, respectively. This is no longer the case: there is
now just the underlying mux pane which is referred to in these docs as `Pane`
for the sake of simplicity.
---


A Pane object is typically passed to your code via an event callback.  A Pane
object is a handle to a live instance of a Pane that is known to the wezterm
process.  A Pane object tracks the pseudo terminal (or real serial terminal)
and associated process(es) and the parsed screen and scrollback.

A Pane object can be used to send input to the associated processes and
introspect the state of the terminal emulation for that pane.

## Available methods


