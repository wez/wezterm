# `text_blink_rate_rapid`

*Since: 20210814-124438-54e29167*

Specifies how often blinking text (rapid speed) transitions between visible
and invisible, expressed in milliseconds.  Setting this to 0 disables rapid text
blinking.  Note that this value is approximate due to the way that the system
event loop schedulers manage timers; non-zero values will be at least the
interval specified with some degree of slop.

```lua
return {
  text_blink_rate_rapid = 250
}
```

