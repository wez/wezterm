---
tags:
  - appearance
---
# `text_blink_rate_rapid`

{{since('20210814-124438-54e29167')}}

Specifies how often blinking text (rapid speed) transitions between visible
and invisible, expressed in milliseconds.  Setting this to 0 disables rapid text
blinking.  Note that this value is approximate due to the way that the system
event loop schedulers manage timers; non-zero values will be at least the
interval specified with some degree of slop.

```lua
config.text_blink_rate_rapid = 250
```

{{since('20220319-142410-0fcdea07')}}

Blinking is no longer a binary blink, but interpolates between invisible and
visible text using an easing function.  See
[text_blink_rapid_ease_in](text_blink_rapid_ease_in.md) and
[text_blink_rapid_ease_out](text_blink_rapid_ease_out.md) for more information.

