# `cursor_blink_rate`

Specifies how often a blinking cursor transitions between visible and
invisible, expressed in milliseconds.  Setting this to 0 disables blinking.

Note that this value is approximate due to the way that the system event loop
schedulers manage timers; non-zero values will be at least the interval
specified with some degree of slop.

It is recommended to avoid blinking cursors when on battery power, as it is
relatively costly to keep re-rendering for the blink!

```lua
return {
  cursor_blink_rate = 800,
}
```
