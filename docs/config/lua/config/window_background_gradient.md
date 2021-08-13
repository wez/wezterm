# window_background_gradient

*Since: nightly builds only*

Dynamically generates a `window_background_image` from the provided gradient
specification.  When `window_background_gradient` is configured, the value
for `window_background_image` is ignored.

Linear gradients with vertical or horizontal orientation are supported:

```lua
return {
  window_background_gradient = {
    -- Can be "Vertical" or "Horizontal".  Specifies the direction
    -- in which the color gradient varies.  The default is "Horizontal",
    -- with the gradient going from left-to-right
    orientation = "Vertical",

    -- Specifies the set of colors that are interpolated in the gradient.
    -- Accepts CSS style color specs, from named colors, through rgb
    -- strings and more
    colors = {
      "#0f0c29",
      "#302b63",
      "#24243e"
    },

    -- Specifies the interpolation style to be used.
    -- "Linear", "Basis" and "CatmullRom" as supported.
    -- The default is "Linear".
    interpolation = "Linear",

    -- How the colors are blended in the gradient.
    -- "Rgb", "LinearRgb", "Hsv" and "Oklab" are supported.
    -- The default is "Rgb".
    blend = "Rgb",

    -- To avoid vertical color banding for horizontal gradients, the
    -- gradient position is randomly shifted by up to the `noise` value
    -- for each pixel.
    -- Smaller values, or 0, will make bands more prominent.
    -- The default value is 64 which gives decent looking results
    -- on a retina macbook pro display.
    -- noise = 64,

    -- By default, the gradient smoothly transitions between the colors.
    -- You can adjust the sharpness by specifying the segment_size and
    -- segment_smoothness parameters.
    -- segment_size configures how many segments are present.
    -- segment_smoothness is how hard the edge is; 0.0 is a hard edge,
    -- 1.0 is a soft edge.

    -- segment_size = 11,
    -- segment_smoothness = 0.0,
  },
}
```

Gradients are implemented using the `colorgrad` crate.
Take a look at <https://github.com/mazznoer/colorgrad-rs#using-web-color-format>
for some usage examples and additional information about gradients.

