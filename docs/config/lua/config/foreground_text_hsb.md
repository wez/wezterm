# foreground_text_hsb

*Since: nightly builds only*

Configures a Hue, Saturation, Brightness transformation that is applied to
monochrome glyphs.

The transform works by converting the RGB colors to HSV values and
then multiplying the HSV by the numbers specified in `foreground_text_hsb`.

Modifying the hue changes the hue of the color by rotating it through the color
wheel. It is not as useful as the other components, but is available "for free"
as part of the colorspace conversion.

Modifying the saturation can add or reduce the amount of "colorfulness". Making
the value smaller can make it appear more washed out.

Modifying the brightness can be used to dim or increase the perceived amount of
light.

The range of these values is 0.0 and up; they are used to multiply the existing
values, so the default of 1.0 preserves the existing component, whilst 0.5 will
reduce it by half, and 2.0 will double the value.

<img src="../../../screenshots/foreground-text-hsb-1-1-1.png">
<img src="../../../screenshots/foreground-text-hsb-1-1.5-1.png">
<img src="../../../screenshots/foreground-text-hsb-1-1-1.5.png">
<img src="../../../screenshots/foreground-text-hsb-1.5-1-1.png">
