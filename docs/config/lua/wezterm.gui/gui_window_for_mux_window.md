# `wezterm.gui.gui_window_for_mux_window(window_id)`

{{since('20220807-113146-c2fee766')}}

Attempts to resolve a mux window to its corresponding [Gui Window](../window/index.md).

This may not succeed for a couple of reasons:

* If called by the multiplexer daemon, there is no gui, so this will never succeed
* If the mux window is part of a workspace that is not the active workspace
