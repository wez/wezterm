---
tags:
  - mouse
---
# `swallow_mouse_click_on_window_focus`

{{since('20220319-142410-0fcdea07')}}

When set to `true`, clicking on a wezterm window will focus it.

When set to `false`, clicking on a wezterm window will focus it and then pass
through the click to the pane where the
[swallow_mouse_click_on_pane_focus](swallow_mouse_click_on_pane_focus.md)
option will further modify mouse event processing.

The default is `true` on macOS but `false` on other systems.
