---
tags:
  - appearance
  - text_cursor
---
# `default_cursor_style = "SteadyBlock"`

Specifies the default cursor style.  Various escape sequences
can override the default style in different situations (eg:
an editor can change it depending on the mode), but this value
controls how the cursor appears when it is reset to default.
The default is `SteadyBlock`.

Acceptable values are `SteadyBlock`, `BlinkingBlock`,
`SteadyUnderline`, `BlinkingUnderline`, `SteadyBar`,
and `BlinkingBar`.

```lua
config.default_cursor_style = 'SteadyBlock'
```

Using a blinking style puts more load on the graphics subsystem.
[animation_fps](animation_fps.md) can be used to tune the frame
rate used for easing in the blink animation.
