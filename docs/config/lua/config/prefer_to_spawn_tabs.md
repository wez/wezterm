---
tags:
  - spawn
---

# `prefer_to_spawn_tabs = false`

{{since('20240203-110809-5046fc22')}}

If set to `true`, launching a new instance of `wezterm` will prefer to
spawn a new tab when it is able to connect to your already-running GUI
instance.

Otherwise, it will spawn a new window.

The default value for this option is `false`.
