# `wezterm.gui.get_appearance()`

*Since: nightly builds only*

This function returns the appearance of the window environment.  The appearance
can be one of the following 4 values:

* `"Light"` - the normal appearance, with dark text on a light background
* `"Dark"` - "dark mode", with predominantly dark colors and probably a lighter, lower contrasting, text color on a dark background
* `"LightHighContrast"` - light mode but with high contrast colors (not reported on all systems)
* `"DarkHighContrast"` - dark mode but with high contrast colors (not reported on all systems)

wezterm is able to detect when the appearance has changed and will generate a
[window-config-reloaded](../window-events/window-config-reloaded.md) event for
each window.

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

return {
  color_scheme = scheme_for_appearance(wezterm.gui.get_appearance()),
}
```

### Wayland GNOME Appearance

wezterm uses [XDG Desktop
Portal](https://flatpak.github.io/xdg-desktop-portal/) to determine the
appearance in a desktop-environment independent way.

