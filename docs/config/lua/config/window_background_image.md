---
tags:
  - appearance
  - background
---
# `window_background_image`

![Screenshot](../../../screenshots/wezterm-vday-screenshot.png)

{{since('20201031-154415-9614e117')}}

You can attach an image to the background of the wezterm window:
```lua
config.window_background_image = '/path/to/wallpaper.jpg'
```

If the path is a relative path then it will be expanded relative to the
directory containing your `wezterm.lua` config file.

PNG, JPEG, GIF, BMP, ICO, TIFF, PNM, DDS, TGA and farbfeld files can be
loaded. Animated GIF and PNG files will animate while the window has focus.

The image will be scaled to fit the window contents. Very large images may
decrease render performance and take up VRAM from the GPU, so you may wish to
resize the image file before using it.

You can optionally transform the background image with a hue, saturation,
brightness multiplier, for example, to darken a bright wallpaper so that text
remains readable.
See [window_background_image_hsb](window_background_image_hsb.md).

## Relationship with other options

The background image is ignored when
[window_background_gradient](window_background_gradient.md) is set.

To have control over scaling, tiling/repeating, scrolling behavior and more,
take a look at the more powerful [background](background.md) configuration
option.
