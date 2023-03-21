# `wezterm.color.extract_colors_from_image(filename [,params])`

{{since('20220807-113146-c2fee766')}}

This function loads an image from the specified filename and analyzes it to
determine a set of distinct colors present in the image, ordered by how often a
given color is found in the image, descending.  So if an image is predominantly
black with a bit of white, then black will be listed first in the returned
array.

This is potentially useful if you wish to generate a color scheme to match
an image, for example.

The default is to extract 16 colors from an image:

```
> wezterm.color.extract_colors_from_image("/wallpapers/neon-nights.jpeg")
[
    "#060a14",
    "#7393d4",
    "#9f475a",
    "#305e73",
    "#4f4660",
    "#958193",
    "#c76199",
    "#689ba6",
    "#1a344e",
    "#4c2633",
    "#b17c75",
    "#854a7a",
    "#3876aa",
    "#d75f75",
    "#231725",
    "#79606f",
]
```

The analysis is relatively expensive and can take several seconds if
used on a full 4K image file.  To reduce the runtime, wezterm will by
default scale down the image and skip over nearby pixels.  The results
of the analysis will be cached to avoid repeating the same work each
time the configuration is re-evaluated.

You can specify optional parameters in a parameter table:

```
> wezterm.color.extract_colors_from_image("/wallpaper/neon-nights.jpeg", {
  num_colors=16,
  threshold=75
})
[
    "#060a14",
    "#48afb7",
    "#d75f74",
    "#5c6795",
    "#a88e67",
    "#64313f",
    "#639cdf",
    "#356e76",
    "#c467c5",
    "#8e8490",
    "#2b3f54",
    "#90537e",
    "#2f233c",
    "#b08279",
    "#97a16b",
    "#8f84be",
]
```

The following fields are allowed in the parameter table:

* `fuzziness` - skip this many pixels when sampling colors, to avoid adding
  candidate colors that are likely similar to each other. Default is `5`.
* `num_colors` - how many colors should be extracted. Default is `16`.
  Set to `0` to find all distinct colors.
* `max_width`, `max_height` - the image will be resized (respecting its aspect
  ratio) to fit within these dimensions to reduce the number of pixels that
  need to be analyzed.  Default is `640` by `480`.
* `min_brightness` - the minimum allowed brightness level you'd like to accept.
  Brightness has the range `0` through `100` with `100` being brightest.
  Useful to exclude very dark colors from the returned palette.  Default is
  `0`.
* `max_brightness` - the maximum allowed brightness level you'd like to accept.
  Useful to exclude very bright colors from the returned palette.  Default is
  `90`.
* `threshold` - colors are compared using `CIEDE2000` DeltaE which produces
  values in the range `0` through `100`.  Smaller values are more similar,
  larger values are more different.  If the computed DeltaE is smaller than
  your `threshold` parameter then the color candidate will be added to the
  returned set of colors.
* `min_contrast` - if set non-zero, in addition to the DeltaE constraint,
  colors must have a contrast ratio of at least `min_contrast`. The default
  is `0`.

If fewer than the requested `num_colors` were found, the threshold will be
repeatedly reduced to increase the set of candidate colors until the threshold
falls below the human perceptible range. If after that fewer than the requested
`num_colors` were found, an error is raised.

When `min_contrast` is in use and fewer than `num_colors` matching colors are
found, `min_contrast` is *not* automatically relaxed when retrying with a lower
`threshold`.

This example computes a color palette for the terminal based on some other image file:

```lua
local wezterm = require 'wezterm'

local colors = wezterm.color.extract_colors_from_image '/path/to/image/jpeg'
local ansi = {}
local brights = {}

for idx, color in ipairs(colors) do
  if idx <= 8 then
    ansi[idx] = color
  else
    brights[idx - 8] = color
  end
end

return {
  colors = {
    ansi = ansi,
    brights = brights,
  },
}
```
