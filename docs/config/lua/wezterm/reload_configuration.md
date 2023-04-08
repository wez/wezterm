---
title: wezterm.reload_configuration
tags:
 - reload
---
# `wezterm.reload_configuration()`

{{since('20220807-113146-c2fee766')}}

Immediately causes the configuration to be reloaded and re-applied.

If you call this at the file scope in your config you will create
an infinite loop that renders wezterm unresponsive, so don't do that!

The intent is for this to be used from an event or timer callback function.
