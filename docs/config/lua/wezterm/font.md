# `wezterm.font(family [, attributes])`

This function constructs a lua table that corresponds to the internal `FontAttributes`
struct that is used to select a single named font:

```lua
local wezterm = require 'wezterm';

return {
  font = wezterm.font("JetBrains Mono"),
}
```

The first parameter is the name of the font; the name can be one of the following types of names:

* The font family name, eg: `"JetBrains Mono"`.  The family name doesn't include any style information (such as weight, stretch or italic), which can be specified via the *attributes* parameter.  This is the recommended name to use for the font, as it the most compatible way to resolve an installed font.
* The computed *full name*, which is the family name with the sub-family (which incorporates style information) appended, eg: `"JetBrains Mono Regular"`.
* (Since 20210502-154244-3f7122cb) The *postscript name*, which is an ostensibly unique name identifying a given font and style that is encoded into the font by the font designer.

When specifying a font using its family name, the second *attributes* parameter
is an optional table that can be used to specify style attributes; the
following keys are allowed:

* `bold` - whether to select a bold variant of the font (default: `false`)
* `italic` - whether to select an italic variant of the font (default: `false`)

When attributes are specified, the font must match both the family name and attributes in order to be selected.

```lua
local wezterm = require 'wezterm';

return {
  font = wezterm.font("JetBrains Mono", {bold=true}),
}
```

*Since: 20210502-130208-bff6815d*

It is now possible to specify both font weight and font stretch when matching fonts:

* `stretch` - specifies the font stretch to select.  The default value is `"Normal"`, and possible values are `"UltraCondensed"`, `"ExtraCondensed"`, `"Condensed"`, `"SemiCondensed"`, `"Normal"`, `"SemiExpanded"`, `"Expanded"`, `"ExtraExpanded"`, `"UltraExpanded"`.
* `weight` - specifies the weight of the font with more precision than `bold`.  The default value is `"Regular"`, and possible values are `"Thin"`, `"ExtraLight"`, `"Light"`, `"DemiLight"`, `"Book"`, `"Regular"`, `"Medium"`, `"DemiBold"`, `"Bold"`, `"ExtraBold"`, `"Black"`, and `"ExtraBlack"`.
* `bold` - has been superseded by the new `weight` parameter and will be eventually removed.  For compatibility purposes, specifying `bold=true` is equivalent to specifying `weight="Bold"`.

These parameters are passed to the system font locator when resolving
the font, which will apply system-specific rules to resolve the font.

When resolving fonts from [font_dirs](../config/font_dirs.md), wezterm follows CSS Fonts
Level 3 compatible font matching, which tries to exactly match the specified
attributes, but allows for locating a close match within the specified font
family.

```lua
local wezterm = require 'wezterm';

return {
  font = wezterm.font('Iosevka Term', {stretch="Expanded", weight="Regular"}),
}
```

An alternative form of specifying the font can be used, where the family and the attributes
are combined in the same lua table.  This form is most useful when used together with
[wezterm.font_with_fallback](font_with_fallback.md) when you want to specify precise
weights for the different fallback fonts:

```lua
local wezterm = require 'wezterm';

return {
  font = wezterm.font({family='Iosevka Term', stretch="Expanded", weight="Regular"}),
}
```

