### Advanced Font Shaping Options

The `harfbuzz_features` option allows specifying the features to enable when
using harfbuzz for font shaping.

There is some light documentation here:
<https://harfbuzz.github.io/shaping-opentype-features.html>
but it boils down to allowing opentype feature names to be specified
using syntax similar to the CSS font-feature-settings options:
<https://developer.mozilla.org/en-US/docs/Web/CSS/font-feature-settings>.
The OpenType spec lists a number of features here:
<https://docs.microsoft.com/en-us/typography/opentype/spec/featurelist>

Options of likely interest will be:

* `calt` - <https://docs.microsoft.com/en-us/typography/opentype/spec/features_ae#tag-calt>
* `clig` - <https://docs.microsoft.com/en-us/typography/opentype/spec/features_ae#tag-clig>

If you want to disable ligatures in most fonts, then you may want to
use a setting like this:

```lua
return {
  harfbuzz_features = { 'calt=0', 'clig=0', 'liga=0' },
}
```

Some fonts make available extended options via stylistic sets.
If you use the [Fira Code font](https://github.com/tonsky/FiraCode),
it lists available stylistic sets here:
<https://github.com/tonsky/FiraCode/wiki/How-to-enable-stylistic-sets>

and you can set them in wezterm:

```lua
return {
  -- Use this for a zero with a dot rather than a line through it
  -- when using the Fira Code font
  harfbuzz_features = { 'zero' },
}
```

*Since: 20220101-133340-7edc5b5a*

You can specify `harfbuzz_features` on a per-font basis, rather than
globally for all fonts:

```lua
local wezterm = require 'wezterm'
return {
  font = wezterm.font {
    family = 'JetBrains Mono',
    harfbuzz_features = { 'calt=0', 'clig=0', 'liga=0' },
  },
}
```

and this example disables ligatures for JetBrains Mono,
but keeps the default for the other fonts in the fallback:

```lua
local wezterm = require 'wezterm'

return {
  font = wezterm.font_with_fallback {
    {
      family = 'JetBrains Mono',
      weight = 'Medium',
      harfbuzz_features = { 'calt=0', 'clig=0', 'liga=0' },
    },
    { family = 'Terminus', weight = 'Bold' },
    'Noto Color Emoji',
  },
}
```

