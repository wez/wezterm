# `text_blink_rate`

*Since: nightly builds only*

Specifies how often blinking text (normal speed) transitions between visible
and invisible, expressed in milliseconds.  Setting this to 0 disables slow text
blinking.  Note that this value is approximate due to the way that the system
event loop schedulers manage timers; non-zero values will be at least the
interval specified with some degree of slop.

```lua
return {
  text_blink_rate = 500
}
```
