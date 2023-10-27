# `wezterm.mux.get_tab(TAB_ID)`

{{since('20220624-141144-bd1b7c5d')}}

Given a tab ID, verifies that the ID is a valid tab known to the mux
and returns a [MuxTab](../MuxTab/index.md) object that can be used to
operate on the tab.

This is useful for situations where you have obtained a tab id from
some other source and want to use the various `MuxTab` methods with it.

