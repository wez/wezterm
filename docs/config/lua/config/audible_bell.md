---
tags:
  - bell
---
# `audible_bell`

{{since('20211204-082213-a66c61ee9')}}

When the BEL ascii sequence is sent to a pane, the bell is "rung" in that pane.

You may choose to configure the `audible_bell` option to change the sound
that wezterm makes when the bell rings.

The follow are possible values:

* `"SystemBeep"` - perform the system beep or alert sound. This is the default. On Wayland systems, which have no system beep function, it does not produce a sound.
* `"Disabled"` - don't make a sound


See also [visual_bell](visual_bell.md) and [bell event](../window-events/bell.md)

