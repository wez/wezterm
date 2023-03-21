# `wezterm.mux.get_pane(PANE_ID)`

{{since('20220624-141144-bd1b7c5d')}}

Given a pane ID, verifies that the ID is a valid pane known to the mux
and returns a [Pane](../pane/index.md) object that can be used to
operate on the pane.

This is useful for situations where you have obtained a pane id from
some other source and want to use the various `Pane` methods with it.

