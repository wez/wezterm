---
tags:
  - appearance
  - background
---
# `background`

{{since('20220624-141144-bd1b7c5d')}}

The `background` config option allows you to compose a number of layers to
produce the background content in the terminal.

Layers can be image files, gradients or solid blocks of color. Layers composite
over each other based on their alpha channel. Images in layers can be made to
fill the viewport or to tile, and also to scroll with optional parallax as the
viewport is scrolled.

This video demonstrates the use of multiple layers to produce a rich video game
style parallax background; the configuration used for this is shown as an
example at the bottom of this page:

<video width="80%" controls src="../../../screenshots/wezterm-parallax-2.mp4" loop></video>

The `background` option is a table that lists the desired layers starting with
the deepest/back-most layer.  Subsequent layers are composited over the top of
preceding layers.


## Layer Definition

A layer is a lua table with the following fields:

* `source` - defines the source of the layer texture data. See below for source definitions
* `attachment` - controls whether the layer is fixed to the viewport or moves as it scrolls. Can be:
    * `"Fixed"` (the default) to not move as the window scrolls,
    * `"Scroll"` to scroll 1:1 with the number of pixels scrolled in the viewport,
    * `{Parallax=0.1}` to scroll 1:10 with the number of pixels scrolled in the viewport.
* `repeat_x` - controls whether the image is repeated in the x-direction. Can be one of:
    * `"Repeat"` - Repeat as much as possible to cover the area. The last image will be clipped if it doesn't fit.  This is the default.
    * `"Mirror"` - Like `"Repeat"` except that the image is alternately mirrored which can make images that don't tile seamlessly look a bit better when repeated
    * `"NoRepeat"` - the image is not repeated.
* `repeat_x_size` - Normally, when repeating, the image is tiled based on its width such that each copy of the image is immediately adjacent to the preceding instance.  You may set `repeat_x_size` to a different value to increase or decrease the space between the repeated instances.  Accepts:
    * number values in pixels,
    * string values like `"100%"` to specify a size relative to the viewport,
    * `"10cell"` to specify a size based on the terminal cell metrics.
* `repeat_y` - like `repeat_x` but affects the y-direction.
* `repeat_y_size` - like `repeat_x_size` but affects the y-direction.
* `vertical_align` - controls the initial vertical position of the layer, relative to the viewport:
    * `"Top"` (the default),
    * `"Middle"`,
    * `"Bottom"`
* `vertical_offset` - specify an offset from the initial vertical position.  Accepts:
    * number values in pixels,
    * string values like `"100%"` to specify a size relative to the viewport,
    * `"10cell"` to specify a size based on terminal cell metrics.
* `horizontal_align` - controls the initial horizontal position of the layer, relative to the viewport:
    * `"Left"` (the default),
    * `"Center"`
    * `"Right"`
* `horizontal_offset` - like `vertical_offset` but applies to the x-direction.
* `opacity` - a number in the range `0` through `1.0` inclusive that is multiplied with the alpha channel of the source to adjust the opacity of the layer. The default is `1.0` to use the source alpha channel as-is. Using a smaller value makes the layer less opaque/more transparent.
* `hsb` - a hue, saturation, brightness transformation that can be used to adjust those attributes of the layer. See [foreground_text_hsb](foreground_text_hsb.md) for more information about this kind of transform.
* `height` - controls the height of the image. The following values are accepted:
    * `"Cover"` (this is the default) - Scales the image, preserving aspect ratio, to the smallest possible size to fill the viewport, leaving no empty space.  If the aspect ratio of the viewport differs from the image, the image is cropped.
    * `"Contain"` - Scales the image as large as possible without cropping or stretching. If the viewport is larger than the image, tiles the image unless `repeat_y` is set to `"NoRepeat"`.
    * `123` - specifies a height of `123` pixels
    * `"50%"` - specifies a size of `50%` of the viewport height
    * `"2cell"` - specifies a size equivalent to `2` rows
* `width` - controls the width of the image. Same details as `height` but applies to the x-direction.

## Source Definition

A source can be one of the following:

* `{File="/path/to/file.png"}` - load the specified image file.  PNG, JPEG,
  GIF, BMP, ICO, TIFF, PNM, DDS, TGA and farbfeld files can be loaded.
  Animated GIF and PNG files will animate while the window has focus.
* `{File={path="/path/to/anim.gif", speed=0.2}}` - load the specified image file, which is an animated gif, and adjust the animation speed to 0.2 times its normal speed.
* `{Gradient={preset="Warm"}}` - generate a gradient. The gradient definitions
  are the same as those allowed for [window_background_gradient](window_background_gradient.md).
* `{Color="black"}` - generate an image with the specified color.

## Relationship with other config options

Specifying the following options:

* `window_background_gradient`
* `window_background_image`
* `window_background_opacity`
* `window_background_image_hsb`

will implicitly prepend a layer to the `background` configuration with width
and height set to 100%.

It is recommended that you migrate to the newer `background` rather than mixing
both the older and the newer configuration options.

## Parallax Example

This example uses these [Alien Space Ship Background - Parallax -
Repeatable
(Vertical)](https://www.gameartguppy.com/shop/space-ship-background-repeatable-vertical/)
assets to demonstrate most of the available features of `background`. That asset pack includes a background layer and a number of overlays. The overlays are positioned at varying offsets with differing parallax to provide a greater sense of depth.

The video at the top of this page demonstrate this configuration in action.

```lua
-- The art is a bit too bright and colorful to be useful as a backdrop
-- for text, so we're going to dim it down to 10% of its normal brightness
local dimmer = { brightness = 0.1 }

config.enable_scroll_bar = true
config.min_scroll_bar_height = '2cell'
config.colors = {
  scrollbar_thumb = 'white',
}
config.background = {
  -- This is the deepest/back-most layer. It will be rendered first
  {
    source = {
      File = '/Alien_Ship_bg_vert_images/Backgrounds/spaceship_bg_1.png',
    },
    -- The texture tiles vertically but not horizontally.
    -- When we repeat it, mirror it so that it appears "more seamless".
    -- An alternative to this is to set `width = "100%"` and have
    -- it stretch across the display
    repeat_x = 'Mirror',
    hsb = dimmer,
    -- When the viewport scrolls, move this layer 10% of the number of
    -- pixels moved by the main viewport. This makes it appear to be
    -- further behind the text.
    attachment = { Parallax = 0.1 },
  },
  -- Subsequent layers are rendered over the top of each other
  {
    source = {
      File = '/Alien_Ship_bg_vert_images/Overlays/overlay_1_spines.png',
    },
    width = '100%',
    repeat_x = 'NoRepeat',

    -- position the spins starting at the bottom, and repeating every
    -- two screens.
    vertical_align = 'Bottom',
    repeat_y_size = '200%',
    hsb = dimmer,

    -- The parallax factor is higher than the background layer, so this
    -- one will appear to be closer when we scroll
    attachment = { Parallax = 0.2 },
  },
  {
    source = {
      File = '/Alien_Ship_bg_vert_images/Overlays/overlay_2_alienball.png',
    },
    width = '100%',
    repeat_x = 'NoRepeat',

    -- start at 10% of the screen and repeat every 2 screens
    vertical_offset = '10%',
    repeat_y_size = '200%',
    hsb = dimmer,
    attachment = { Parallax = 0.3 },
  },
  {
    source = {
      File = '/Alien_Ship_bg_vert_images/Overlays/overlay_3_lobster.png',
    },
    width = '100%',
    repeat_x = 'NoRepeat',

    vertical_offset = '30%',
    repeat_y_size = '200%',
    hsb = dimmer,
    attachment = { Parallax = 0.4 },
  },
  {
    source = {
      File = '/Alien_Ship_bg_vert_images/Overlays/overlay_4_spiderlegs.png',
    },
    width = '100%',
    repeat_x = 'NoRepeat',

    vertical_offset = '50%',
    repeat_y_size = '150%',
    hsb = dimmer,
    attachment = { Parallax = 0.5 },
  },
}
```
