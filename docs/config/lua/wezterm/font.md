# `wezterm.font(family [, attributes])`

This function constructs a lua table that corresponds to the internal `FontAttributes`
struct that is used to select a single named font:

```lua
local wezterm = require 'wezterm';

return {
  font = wezterm.font("JetBrains Mono"),
}
```

The second parameter is an optional table that can be used to specify some
attributes; the following keys are allowed:

* `bold` - whether to select a bold variant of the font (default: `false`)
* `italic` - whether to select an italic variant of the font (default: `false`)

```lua
local wezterm = require 'wezterm';

return {
  font = wezterm.font("JetBrains Mono", {bold=true}),
}
```

*Since: nightly builds only*

It is now possible to specify both font weight and font width:

* `width` - specifies the font width to select.  The default value is `"Normal"`, and possible values are `"UltraCondensed"`, `"ExtraCondensed"`, `"Condensed"`, `"SemiCondensed"`, `"Normal"`, `"SemiExpanded"`, `"Expanded"`, `"ExtraExpanded"`, `"UltraExpanded"`.
* `weight` - specifies the weight of the font with more precision than `bold`.  The default value is `"Regular"`, and possible values are `"Thin"`, `"ExtraLight"`, `"Light"`, `"DemiLight"`, `"Book"`, `"Regular"`, `"Medium"`, `"DemiBold"`, `"Bold"`, `"ExtraBold"`, `"Black"`, and `"ExtraBlack"`.
* `bold` - has been superseded by the new `weight` parameter and will be eventually removed.  For compatibility purposes, specifying `bold=true` is equivalent to specifying `weight="Bold"`.

Font weight matching will find the closest matching weight that is equal of
heavier to the specified weight.

```lua
local wezterm = require 'wezterm';

return {
  font = wezterm.font('Iosevka Term', {width="Expanded", weight="Regular"}),
}
```
