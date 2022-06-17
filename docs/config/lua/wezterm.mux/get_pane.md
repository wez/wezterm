# `wezterm.mux.get_pane(PANE_ID)`

*Since: nightly builds only*

Given a pane ID, verifies that the ID is a valid pane known to the mux
and returns a [MuxPane](../MuxPane.md) object that can be used to
operate on the pane.

This is useful for situations where you have obtained a pane id from
some other source and want to use the various `MuxPane` methods with it.

