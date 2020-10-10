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


