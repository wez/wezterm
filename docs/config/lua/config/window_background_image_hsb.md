---
tags:
  - appearance
  - background
---
# `window_background_image_hsb`

{{since('20201031-154415-9614e117')}}

Configures a Hue, Saturation, Brightness transformation that is applied to the
[window_background_image](window_background_image.md).

The transform works by converting the RGB values of the image to HSV values and
then multiplying the HSV by the numbers specified in `window_background_image_hsb`.

Modifying the hue changes the hue of the color by rotating it through the color
wheel.
_It is not as useful as the other components, but is available "for free" as part
of the colorspace conversion._

Modifying the saturation can add or reduce the amount of "colorfulness".
Making the value smaller can make it appear more washed out.

Modifying the brightness can be used to dim or increase the perceived amount of
light.

The range of these values is 0.0 and up; they are used to multiply the existing
values, so the default of 1.0 preserves the existing component, whilst 0.5 will
reduce it by half, and 2.0 will double the value.

```lua
config.window_background_image = '/path/to/wallpaper.jpg'

config.window_background_image_hsb = {
  -- Darken the background image by reducing it to 1/3rd
  brightness = 0.3,

  -- You can adjust the hue by scaling its value.
  -- a multiplier of 1.0 leaves the value unchanged.
  hue = 1.0,

  -- You can also adjust the saturation.
  saturation = 1.0,
}
```

The same kind of transformation can be applied to monochrome glyphs with
[foreground_text_hsb](foreground_text_hsb.md).
