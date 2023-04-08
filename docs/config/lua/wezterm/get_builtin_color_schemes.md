---
title: wezterm.get_builtin_color_schemes
tags:
 - color
 - scheme
 - theme
---

# `wezterm.get_builtin_color_schemes()`

{{since('20220101-133340-7edc5b5a')}}

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

wezterm.on('window-config-reloaded', function(window, pane)
  -- If there are no overrides, this is our first time seeing
  -- this window, so we can pick a random scheme.
  if not window:get_config_overrides() then
    -- Pick a random scheme name
    local scheme = schemes[math.random(#schemes)]
    window:set_config_overrides {
      color_scheme = scheme,
    }
  end
end)

return {}
```

This example shows how to take an existing scheme, modify a color, and
then use that new scheme to override the default:

```lua
local wezterm = require 'wezterm'

local scheme = wezterm.get_builtin_color_schemes()['Gruvbox Light']
scheme.background = 'red'

return {
  color_schemes = {
    -- Override the builtin Gruvbox Light scheme with our modification.
    ['Gruvbox Light'] = scheme,

    -- We can also give it a different name if we don't want to override
    -- the default
    ['Gruvbox Red'] = scheme,
  },
  color_scheme = 'Gruvbox Light',
}
```

This example shows how to analyze the colors in the builtin schemes and
use that to select just the dark schemes and then randomly pick one
of those for each new window:

```lua
local wezterm = require 'wezterm'

local function dark_schemes()
  local schemes = wezterm.get_builtin_color_schemes()
  local dark = {}
  for name, scheme in pairs(schemes) do
    -- parse into a color object
    local bg = wezterm.color.parse(scheme.background)
    -- and extract HSLA information
    local h, s, l, a = bg:hsla()

    -- `l` is the "lightness" of the color where 0 is darkest
    -- and 1 is lightest.
    if l < 0.4 then
      table.insert(dark, name)
    end
  end

  table.sort(dark)
  return dark
end

local dark = dark_schemes()

wezterm.on('window-config-reloaded', function(window, pane)
  -- If there are no overrides, this is our first time seeing
  -- this window, so we can pick a random scheme.
  if not window:get_config_overrides() then
    -- Pick a random scheme name

    local scheme = dark[math.random(#dark)]
    window:set_config_overrides {
      color_scheme = scheme,
    }
  end
end)

return {}
```

{{since('20220807-113146-c2fee766')}}

This function moved to
[wezterm.color.get_builtin_schemes()](../wezterm.color/get_builtin_schemes.md)
but can still be called as `wezterm.get_builtin_color_schemes()`. See that page
for more examples.
