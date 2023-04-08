---
tags:
  - appearance
  - bell
---
# `visual_bell`

{{since('20211204-082213-a66c61ee9')}}

When the BEL ascii sequence is sent to a pane, the bell is "rung" in that pane.

You may choose to configure the `visual_bell` option so show a visible representation of the bell event,
by having the background color of the pane briefly change color.

There are four fields to the visual_bell config option:

* `fade_in_duration_ms` - how long it should take for the bell color to fade in, in milliseconds. The default is 0.
* `fade_out_duration_ms` - how long it should take for the bell color to fade out, in milliseconds. The default is 0.
* `fade_in_function` - an easing function, similar to [CSS easing functions](https://developer.mozilla.org/en-US/docs/Web/CSS/easing-function), that affects how the bell color is faded in.
* `fade_out_function` - an easing function that affects how the bell color is faded out.
* `target` - can be `"BackgroundColor"` (the default) to have the background color of the terminal change when the bell is rung, or `"CursorColor"` to have the cursor color change when the bell is rung.

If the total fade in and out durations are 0, then there will be no visual bell indication.

The bell color is itself specified in your color settings; if not specified, the text foreground color will be used.

The following easing functions are supported:

* `Linear` - the fade happens at a constant rate.
* `Ease` - The fade starts slowly, accelerates sharply, and then slows gradually towards the end. This is the default.
* `EaseIn` - The fade starts slowly, and then progressively speeds up until the end, at which point it stops abruptly.
* `EaseInOut` - The fade starts slowly, speeds up, and then slows down towards the end.
* `EaseOut` - The fade starts abruptly, and then progressively slows down towards the end.
* `{CubicBezier={0.0, 0.0, 0.58, 1.0}}` - an arbitrary cubic bezier with the specified parameters.
* `Constant` - Evaluates as 0 regardless of time. Useful to implement a step transition at the end of the duration. {{since('20220408-101518-b908e2dd', inline=True)}}

The following configuration enables a low intensity visual bell that takes a total of 300ms to "flash" the screen:

```lua
config.visual_bell = {
  fade_in_function = 'EaseIn',
  fade_in_duration_ms = 150,
  fade_out_function = 'EaseOut',
  fade_out_duration_ms = 150,
}
config.colors = {
  visual_bell = '#202020',
}
```

The follow configuration make the cursor briefly flare when the bell is run:

```lua
config.visual_bell = {
  fade_in_duration_ms = 75,
  fade_out_duration_ms = 75,
  target = 'CursorColor',
}
```

See also [audible_bell](audible_bell.md) and [bell event](../window-events/bell.md).
