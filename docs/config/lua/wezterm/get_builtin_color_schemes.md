# `wezterm.get_builtin_color_schemes()`

*Since: 20220101-133340-7edc5b5a*

Returns a lua table keyed by color scheme name and whose values are
the color scheme definition of the builtin color schemes.

This is useful for programmatically deciding things about the scheme
to use based on its color, or for taking a scheme and overriding a
couple of entries just from your `wezterm.lua` configuration file.

This example shows how to make wezterm pick a random color scheme for
each newly created window:

```lua
local wezterm = require 'wezterm'

-- The set of schemes that we like and want to put in our rotation
local schemes = {}
for name, scheme in pairs(wezterm.get_builtin_color_schemes()) do
  table.insert(schemes, name)
end

wezterm.on("window-config-reloaded", function(window, pane)
  -- If there are no overrides, this is our first time seeing
  -- this window, so we can pick a random scheme.
  if not window:get_config_overrides() then
    -- Pick a random scheme name
    local scheme = schemes[math.random(#schemes)]
    window:set_config_overrides({
      color_scheme = scheme
    })
  end
end)

return {
}
```

This example shows how to take an existing scheme, modify a color, and
then use that new scheme to override the default:

```lua
local wezterm = require 'wezterm'

local scheme = wezterm.get_builtin_color_schemes()["Gruvbox Light"]
scheme.background = "red"

return {
  color_schemes = {
    -- Override the builtin Gruvbox Light scheme with our modification.
    ["Gruvbox Light"] = scheme,

    -- We can also give it a different name if we don't want to override
    -- the default
    ["Gruvbox Red"] = scheme,
  },
  color_scheme = "Gruvbox Light",
}
```

