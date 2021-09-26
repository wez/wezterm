# audible_bell

*Since: nightly builds only*

When the BEL ascii sequence is sent to a pane, the bell is "rung" in that pane.

You may choose to configure the `audible_bell` option to change the sound
that wezterm makes when the bell rings.

The follow are possible values:

* `"SystemBeep"` - perform the system beep or alert sound. This is the default. On some systems, it may not produce a beep sound.
* `"Disabled"` - don't make a sound


See also [visual_bell](visual_bell.md).

