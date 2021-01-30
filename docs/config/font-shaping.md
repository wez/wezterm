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
  harfbuzz_features = {"calt=0", "clig=0", "liga=0"},
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
  harfbuzz_features = {"zero"}
}
```


