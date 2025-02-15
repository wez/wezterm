---
tags:
  - appearance
---
# `window_content_alignment`

{{since('nightly')}}

Controls the alignment of the terminal cells inside the window.

When window size is not a multiple of terminal cell size, terminal cells will be slightly smaller than the window, and leave a small gap between the two.
You can use this option to control where the additional gap will be.

The lua table has two fields and following possible values:

* `horizontal`
    * `"Left"` (the default)
    * `"Center"`
    * `"Right"`
* `vertical`
    * `"Top"` (the default)
    * `"Center"`
    * `"Bottom"`

For example, to center the terminal cells:

```lua
config.window_content_alignment = {
  horizontal = 'Center',
  vertical = 'Center',
}
```
