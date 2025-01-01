# `wezterm.gui.get_appearance()`

{{since('20220807-113146-c2fee766')}}

This function returns the appearance of the window environment.  The appearance
can be one of the following 4 values:

* `"Light"` - the normal appearance, with dark text on a light background
* `"Dark"` - "dark mode", with predominantly dark colors and probably a lighter, lower contrasting, text color on a dark background
* `"LightHighContrast"` - light mode but with high contrast colors (not reported on all systems)
* `"DarkHighContrast"` - dark mode but with high contrast colors (not reported on all systems)

wezterm is able to detect when the appearance has changed and will reload the
configuration when that happens.

This example configuration shows how you can have your color scheme
automatically adjust to the current appearance:

```lua
local wezterm = require 'wezterm'

-- wezterm.gui is not available to the mux server, so take care to
-- do something reasonable when this config is evaluated by the mux
function get_appearance()
  if wezterm.gui then
    return wezterm.gui.get_appearance()
  end
  return 'Dark'
end

function scheme_for_appearance(appearance)
  if appearance:find 'Dark' then
    return 'Builtin Solarized Dark'
  else
    return 'Builtin Solarized Light'
  end
end

return {
  color_scheme = scheme_for_appearance(get_appearance()),
}
```

### Wayland GNOME Appearance

wezterm uses [XDG Desktop
Portal](https://flatpak.github.io/xdg-desktop-portal/) to determine the
appearance in a desktop-environment independent way.

