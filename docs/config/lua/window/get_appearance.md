# window:get_appearance()

*Since: nightly builds only*

This method returns the appearance of the window environment.  The appearance
can be one of the following 4 values:

* `"Light"` - the normal appearance, with dark text on a light background
* `"Dark"` - "dark mode", with predominantly dark colors and probably a lighter, lower contrasting, text color on a dark background
* `"LightHighContrast"` - light mode but with high contrast colors
* `"DarkHighContrast"` - dark mode but with high contrast colors

wezterm currently doesn't know how to interrogate the appearance on Wayland
systems, and will always report `"Light"`.

On macOS, X11 and Windows systems, wezterm is able to detect when the
appearance has changed and will generate a
[window-config-reloaded](../events/window-config-reloaded.md) event for each
window.

This example configuration shows how you can have your color scheme
automatically adjust to the current appearance:

```lua
local wezterm = require 'wezterm'

function scheme_for_appearance(appearance)
  if appearance:find("Dark") then
    return "Builtin Solarized Dark"
  else
    return "Builtin Solarized Light"
  end
end

wezterm.on("window-config-reloaded", function(window, pane)
  local overrides = window:get_config_overrides() or {}
  local appearance = window:get_appearance()
  local scheme = scheme_for_appearance(appearance)
  if overrides.color_scheme ~= scheme then
    overrides.color_scheme = scheme
    window:set_config_overrides(overrides)
  end
end)

return {
}
```

